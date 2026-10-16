//! Hot bitmaps: decoded from chunk storage's cold pool, read and
//! changed, a bucket kept only for a chunk with cells, and only the
//! types asked for made hot.
//!
//! `cargo test`

use crate::tests::*;
use bitmap::WORDS;
use bitplane_manager::diagnostics::arena::ArenaStats;
use bitplane_manager::{BitmapArena, BucketKey, NotHot, Write, WriteOp};
use chunk_storage::{Bits4, WidePlane, LayerCodec, LayerType};
use coordinates::{ChunkIndex, SuperchunkIndex};

/// A bitmap turns hot decoded from its chunk's layer, or empty if the
/// chunk has none; cells of it are read and changed through the arena,
/// and a cell of a bitmap not hot is refused.
#[test]
fn hot_bitmaps_hold_their_chunks_cells() {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let cold_storage = storage_with(ORIGIN, 0, &[(LayerType::of_a_bit(1), rectangle_and_circle())], &mut layer_codec);
    let chunk = ChunkIndex(0);
    let drawn_key = BucketKey { layer_type: LayerType::of_a_bit(1), chunk };
    let absent_key = BucketKey { layer_type: LayerType::of_a_bit(2), chunk };

    assert!(bitmap_arena.make_hot(drawn_key, cold_storage.layer(chunk, LayerType::of_a_bit(1)), &mut layer_codec));
    assert!(bitmap_arena.make_hot(absent_key, cold_storage.layer(chunk, LayerType::of_a_bit(2)), &mut layer_codec));
    assert_eq!(bitmap_arena.bucket(drawn_key).expect("hot").cells(), &rectangle_and_circle());
    assert!(bitmap_arena.bucket(absent_key).expect("hot").cells().iter().all(|&word| word == 0));

    let inside_the_circle = cell_in(chunk, (180, 180));
    assert_eq!(bitmap_arena.layer_holds_cell(LayerType::of_a_bit(1), inside_the_circle), Ok(true));
    write(&mut bitmap_arena, LayerType::of_a_bit(1), WriteOp::ClearCell, inside_the_circle);
    assert_eq!(bitmap_arena.layer_holds_cell(LayerType::of_a_bit(1), inside_the_circle), Ok(false));
    // Turning it hot again keeps the change.
    assert!(!bitmap_arena.make_hot(drawn_key, cold_storage.layer(chunk, LayerType::of_a_bit(1)), &mut layer_codec));
    assert_eq!(bitmap_arena.layer_holds_cell(LayerType::of_a_bit(1), inside_the_circle), Ok(false));

    let cold = BucketKey { layer_type: LayerType::of_a_bit(3), chunk };
    assert_eq!(bitmap_arena.layer_holds_cell(LayerType::of_a_bit(3), inside_the_circle), Err(NotHot(cold)));
    assert_eq!(write(&mut bitmap_arena, LayerType::of_a_bit(3), WriteOp::SetCell, inside_the_circle).missed, 1, "a write to a cold bitmap is missed");
}

/// The arena's bitmaps come in order by superchunk, then type, then
/// chunk, superchunks and chunks in Morton order, however they turned
/// hot; a type's alone come superchunk by superchunk.
#[test]
fn buckets_come_by_superchunk_then_type_then_chunk() {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let superchunks = [SuperchunkIndex::from_cartesian(2, 0), ORIGIN, SuperchunkIndex::from_cartesian(1, 0)];
    for layer_type in [LayerType::of_a_bit(9), LayerType::of_a_bit(4)] {
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
    let expected_keys: Vec<BucketKey> = superchunks_in_order
        .into_iter()
        .flat_map(|superchunk| {
            [LayerType::of_a_bit(4), LayerType::of_a_bit(9)]
                .into_iter()
                .flat_map(move |layer_type| places_in_order.map(|place| BucketKey { layer_type, chunk: ChunkIndex::from_superchunk_place(superchunk, place) }))
        })
        .collect();
    assert_eq!(bitmap_arena.hot_bucket_keys().collect::<Vec<_>>(), expected_keys);
    assert_eq!(bitmap_arena.len(), expected_keys.len());
    assert_eq!(bitmap_arena.buckets_of(LayerType::of_a_bit(9)).map(|(chunk, _)| chunk).collect::<Vec<_>>(), chunks_in_order);
    assert_eq!(bitmap_arena.buckets_of(LayerType::of_a_bit(5)).count(), 0);
}

/// A hot bitmap with no cell set keeps no bucket, and reads clear; the
/// first cell set in it makes one, in its place among the others,
/// which keep every cell of theirs; clearing a cell where there is no
/// bucket makes none.
#[test]
fn a_bucket_is_kept_only_for_a_chunk_with_cells() {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let (plain, stage) = (LayerType::of_a_bit(5), WidePlane::<Bits4>::new(40));
    let chunk = |place: u64| ChunkIndex(place);
    for place in 0..16 {
        bitmap_arena.make_hot(BucketKey { layer_type: plain, chunk: chunk(place) }, None, &mut layer_codec);
        bitmap_arena.make_hot_cells(BucketKey { layer_type: stage.layer_type(), chunk: chunk(place) }, None);
    }
    let held_counts = |arena_measured: &BitmapArena| {
        let arena_stats = ArenaStats::measure_arena(arena_measured);
        (arena_stats.hot_bitmaps, arena_stats.buckets, arena_stats.bucket_bytes)
    };
    assert_eq!(held_counts(&bitmap_arena), (32, 0, 0), "hot, and nothing kept");
    let cell = |place: u64, offset_across: i32| chunk(place).top_left().at_cartesian_offset(offset_across, 3).expect("in the chunk");
    assert_eq!((bitmap_arena.layer_holds_cell(plain, cell(9, 5)), bitmap_arena.value(stage, cell(9, 5))), (Ok(false), Ok(0)));
    bitmap_arena.queue_write(plain, Write::cell(cell(9, 5), WriteOp::ClearCell));
    bitmap_arena.queue_write(stage.layer_type(), Write::value(stage, cell(9, 5), 0));
    assert_eq!((bitmap_arena.apply().cells_changed, held_counts(&bitmap_arena)), (0, (32, 0, 0)), "nothing cleared, nothing kept");

    // Cells set chunk by chunk, out of Morton order: each bucket made between the ones there.
    let mut cells_written = Vec::new();
    for (round_index, place) in [12, 3, 7, 0, 15].into_iter().enumerate() {
        for cell_across in 0..=round_index as i32 {
            bitmap_arena.queue_write(plain, Write::cell(cell(place, cell_across), WriteOp::SetCell));
            bitmap_arena.queue_write(stage.layer_type(), Write::value(stage, cell(place, cell_across), 9 + round_index as u32));
            cells_written.push((cell(place, cell_across), 9 + round_index as u32));
        }
        assert_eq!(bitmap_arena.apply().cells_changed, 2 * (round_index as u64 + 1));
        let bitmap = 8 * WORDS as u64;
        assert_eq!(held_counts(&bitmap_arena), (32, 2 * (round_index + 1), (round_index as u64 + 1) * 5 * bitmap), "a bucket a chunk with cells, each as wide as its layer");
        for &(cell, value) in &cells_written {
            assert_eq!((bitmap_arena.layer_holds_cell(plain, cell), bitmap_arena.value(stage, cell)), (Ok(true), Ok(value)), "kept as set, wherever its bucket went");
        }
        assert_eq!((bitmap_arena.superchunk_count(plain, SuperchunkIndex(0)), bitmap_arena.superchunk_count(stage.layer_type(), SuperchunkIndex(0))), (cells_written.len() as u32, cells_written.len() as u32));
    }
    assert_eq!(bitmap_arena.bucket(BucketKey { layer_type: plain, chunk: chunk(7) }).expect("hot").count(), 3);
    assert_eq!(bitmap_arena.bucket(BucketKey { layer_type: plain, chunk: chunk(8) }).expect("hot, with no bucket").count(), 0);
}
