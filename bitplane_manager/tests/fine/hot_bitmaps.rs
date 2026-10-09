//! Hot bitmaps: decoded from chunk storage's cold pool, read and
//! changed, a bucket kept only for a chunk with cells, and only the
//! types asked for made hot.
//!
//! `cargo test`

use crate::tests::*;
use bitmap::WORDS;
use bitplane_manager::diagnostics::arena::ArenaStats;
use bitplane_manager::{BitmapArena, BucketKey, NotHot, Write, WriteOp};
use chunk_storage::{Bits4, Wide, LayerCodec, LayerType};
use coordinates::{ChunkIndex, SuperchunkIndex};

/// A bitmap turns hot decoded from its chunk's layer, or empty if the
/// chunk has none; cells of it are read and changed through the arena,
/// and a cell of a bitmap not hot is refused.
#[test]
fn hot_bitmaps_hold_their_chunks_cells() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let storage = storage_with(ORIGIN, 0, &[(LayerType(1), drawn())], &mut codec);
    let chunk = ChunkIndex(0);
    let drawn_key = BucketKey { layer_type: LayerType(1), chunk };
    let absent_key = BucketKey { layer_type: LayerType(2), chunk };

    assert!(arena.make_hot(drawn_key, storage.layer(chunk, LayerType(1)), &mut codec));
    assert!(arena.make_hot(absent_key, storage.layer(chunk, LayerType(2)), &mut codec));
    assert_eq!(arena.bucket(drawn_key).expect("hot").cells(), &drawn());
    assert!(arena.bucket(absent_key).expect("hot").cells().iter().all(|&word| word == 0));

    let inside_the_circle = cell_in(chunk, (180, 180));
    assert_eq!(arena.holds(LayerType(1), inside_the_circle), Ok(true));
    write(&mut arena, LayerType(1), WriteOp::Unset, inside_the_circle);
    assert_eq!(arena.holds(LayerType(1), inside_the_circle), Ok(false));
    // Turning it hot again keeps the change.
    assert!(!arena.make_hot(drawn_key, storage.layer(chunk, LayerType(1)), &mut codec));
    assert_eq!(arena.holds(LayerType(1), inside_the_circle), Ok(false));

    let cold = BucketKey { layer_type: LayerType(3), chunk };
    assert_eq!(arena.holds(LayerType(3), inside_the_circle), Err(NotHot(cold)));
    assert_eq!(write(&mut arena, LayerType(3), WriteOp::Set, inside_the_circle).missed, 1, "a write to a cold bitmap is missed");
}

/// The arena's bitmaps come in order by superchunk, then type, then
/// chunk, superchunks and chunks in Morton order, however they turned
/// hot; a type's alone come superchunk by superchunk.
#[test]
fn buckets_come_by_superchunk_then_type_then_chunk() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let superchunks = [SuperchunkIndex::from_cartesian(2, 0), ORIGIN, SuperchunkIndex::from_cartesian(1, 0)];
    for layer_type in [LayerType(9), LayerType(4)] {
        for superchunk in superchunks {
            for place in [3, 2, 1, 0] {
                arena.make_hot(BucketKey { layer_type, chunk: ChunkIndex::of(superchunk, place) }, None, &mut codec);
            }
        }
    }
    let superchunks_in_order = [ORIGIN, SuperchunkIndex::from_cartesian(1, 0), SuperchunkIndex::from_cartesian(2, 0)];
    let places_in_order = [0, 1, 2, 3];
    let chunks_in_order: Vec<ChunkIndex> = superchunks_in_order
        .into_iter()
        .flat_map(|superchunk| places_in_order.map(|place| ChunkIndex::of(superchunk, place)))
        .collect();
    let expected: Vec<BucketKey> = superchunks_in_order
        .into_iter()
        .flat_map(|superchunk| {
            [LayerType(4), LayerType(9)]
                .into_iter()
                .flat_map(move |layer_type| places_in_order.map(|place| BucketKey { layer_type, chunk: ChunkIndex::of(superchunk, place) }))
        })
        .collect();
    assert_eq!(arena.keys().collect::<Vec<_>>(), expected);
    assert_eq!(arena.len(), expected.len());
    assert_eq!(arena.run(LayerType(9)).map(|(chunk, _)| chunk).collect::<Vec<_>>(), chunks_in_order);
    assert_eq!(arena.run(LayerType(5)).count(), 0);
}

/// A hot bitmap with no cell set keeps no bucket, and reads clear; the
/// first cell set in it makes one, in its place among the others,
/// which keep every cell of theirs; clearing a cell where there is no
/// bucket makes none.
#[test]
fn a_bucket_is_kept_only_for_a_chunk_with_cells() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let (plain, stage) = (LayerType(5), Wide::<Bits4>::new(40));
    let chunk = |place: u64| ChunkIndex(place);
    for place in 0..16 {
        arena.make_hot(BucketKey { layer_type: plain, chunk: chunk(place) }, None, &mut codec);
        arena.make_hot_cells(BucketKey { layer_type: stage.layer_type(), chunk: chunk(place) }, None);
    }
    let held = |arena: &BitmapArena| {
        let stats = ArenaStats::of(arena);
        (stats.hot_bitmaps, stats.buckets, stats.bucket_bytes)
    };
    assert_eq!(held(&arena), (32, 0, 0), "hot, and nothing kept");
    let cell = |place: u64, across: i32| chunk(place).top_left().offset(across, 3).expect("in the chunk");
    assert_eq!((arena.holds(plain, cell(9, 5)), arena.value(stage, cell(9, 5))), (Ok(false), Ok(0)));
    arena.queue(plain, Write::cell(cell(9, 5), WriteOp::Unset));
    arena.queue(stage.layer_type(), Write::value(stage, cell(9, 5), 0));
    assert_eq!((arena.apply().changed, held(&arena)), (0, (32, 0, 0)), "nothing cleared, nothing kept");

    // Cells set chunk by chunk, out of Morton order: each bucket made between the ones there.
    let mut set = Vec::new();
    for (round, place) in [12, 3, 7, 0, 15].into_iter().enumerate() {
        for across in 0..=round as i32 {
            arena.queue(plain, Write::cell(cell(place, across), WriteOp::Set));
            arena.queue(stage.layer_type(), Write::value(stage, cell(place, across), 9 + round as u32));
            set.push((cell(place, across), 9 + round as u32));
        }
        assert_eq!(arena.apply().changed, 2 * (round as u64 + 1));
        let bitmap = 8 * WORDS as u64;
        assert_eq!(held(&arena), (32, 2 * (round + 1), (round as u64 + 1) * 5 * bitmap), "a bucket a chunk with cells, each as wide as its layer");
        for &(cell, value) in &set {
            assert_eq!((arena.holds(plain, cell), arena.value(stage, cell)), (Ok(true), Ok(value)), "kept as set, wherever its bucket went");
        }
        assert_eq!((arena.superchunk_count(plain, SuperchunkIndex(0)), arena.superchunk_count(stage.layer_type(), SuperchunkIndex(0))), (set.len() as u32, set.len() as u32));
    }
    assert_eq!(arena.bucket(BucketKey { layer_type: plain, chunk: chunk(7) }).expect("hot").count(), 3);
    assert_eq!(arena.bucket(BucketKey { layer_type: plain, chunk: chunk(8) }).expect("hot, with no bucket").count(), 0);
}
