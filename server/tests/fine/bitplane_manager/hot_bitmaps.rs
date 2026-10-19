//! Hot bitmaps: decoded from chunk storage's cold pool, read and
//! changed, and only the types asked for made hot. A bucket kept only
//! for a chunk with cells: `tests/unit/buckets.rs`, which counts them
//! from inside the crate.
//!
//! `cargo test`

use crate::chunks::*;
use crate::test_types::TestTypes;
use crate::bitplane_manager::{BitmapArena, BucketKey, NotHot, WriteOp};
use crate::chunk_storage::LayerCodec;
use utilities::coordinates::{ChunkIndex, SuperchunkIndex};

/// A bitmap turns hot decoded from its chunk's layer, or empty if the
/// chunk has none; cells of it are read and changed through the arena,
/// and a cell of a bitmap not hot is refused.
#[test]
fn hot_bitmaps_hold_their_chunks_cells() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let cold_storage = storage_with(ORIGIN, 0, &[(types.layers[0], rectangle_and_circle())], &mut layer_codec);
    let chunk = ChunkIndex(0);
    let drawn_key = BucketKey { layer_type: types.layers[0], chunk };
    let absent_key = BucketKey { layer_type: types.layers[1], chunk };

    assert!(bitmap_arena.make_hot(drawn_key, cold_storage.layer(chunk, types.layers[0]), &mut layer_codec));
    assert!(bitmap_arena.make_hot(absent_key, cold_storage.layer(chunk, types.layers[1]), &mut layer_codec));
    assert_eq!(bitmap_arena.bucket(drawn_key).expect("hot").cells(), &rectangle_and_circle());
    assert!(bitmap_arena.bucket(absent_key).expect("hot").cells().iter().all(|&word| word == 0));

    let inside_the_circle = cell_in(chunk, (180, 180));
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], inside_the_circle), Ok(true));
    write(&mut bitmap_arena, types.layers[0], WriteOp::ClearCell, inside_the_circle);
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], inside_the_circle), Ok(false));
    // Turning it hot again keeps the change.
    assert!(!bitmap_arena.make_hot(drawn_key, cold_storage.layer(chunk, types.layers[0]), &mut layer_codec));
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], inside_the_circle), Ok(false));

    let cold = BucketKey { layer_type: types.layers[2], chunk };
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[2], inside_the_circle), Err(NotHot(cold)));
    assert_eq!(write(&mut bitmap_arena, types.layers[2], WriteOp::SetCell, inside_the_circle).missed, 1, "a write to a cold bitmap is missed");
}

/// The arena's bitmaps come in order by superchunk, then type, then
/// chunk, superchunks and chunks in Morton order, however they turned
/// hot; a type's alone come superchunk by superchunk.
#[test]
fn buckets_come_by_superchunk_then_type_then_chunk() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let superchunks = [SuperchunkIndex::from_cartesian(2, 0), ORIGIN, SuperchunkIndex::from_cartesian(1, 0)];
    for layer_type in [types.layers[5], types.layers[3]] {
        for superchunk in superchunks {
            for place in [3, 2, 1, 0] {
                bitmap_arena.make_hot(BucketKey { layer_type, chunk: ChunkIndex::from_superchunk_place(superchunk, place) }, None, &mut layer_codec);
            }
        }
    }
    let superchunks_in_order = [ORIGIN, SuperchunkIndex::from_cartesian(1, 0), SuperchunkIndex::from_cartesian(2, 0)];
    let places_in_order = [0, 1, 2, 3];
    let chunks_in_order: Vec<ChunkIndex> = superchunks_in_order
        .into_iter()
        .flat_map(|superchunk| places_in_order.map(|place| ChunkIndex::from_superchunk_place(superchunk, place)))
        .collect();
    let mut types_in_order = [types.layers[3], types.layers[5]];
    types_in_order.sort_unstable();
    let expected_keys: Vec<BucketKey> = superchunks_in_order
        .into_iter()
        .flat_map(|superchunk| {
            types_in_order
                .into_iter()
                .flat_map(move |layer_type| places_in_order.map(|place| BucketKey { layer_type, chunk: ChunkIndex::from_superchunk_place(superchunk, place) }))
        })
        .collect();
    assert_eq!(bitmap_arena.hot_bucket_keys().collect::<Vec<_>>(), expected_keys);
    assert_eq!(bitmap_arena.len(), expected_keys.len());
    assert_eq!(bitmap_arena.buckets_of(types.layers[5]).map(|(chunk, _)| chunk).collect::<Vec<_>>(), chunks_in_order);
    assert_eq!(bitmap_arena.buckets_of(types.layers[4]).count(), 0);
}
