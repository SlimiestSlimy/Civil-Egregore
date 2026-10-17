//! The arena's buckets, counted from inside the crate: what no test
//! from outside can see. Built into the crate's own tests alone
//! (`src/lib.rs`).

use crate::{BitmapArena, BucketKey, Write, WriteOp};
use bitmap::WORDS;
use chunk_storage::{Bits4, LayerCodec, LayerType, WidePlane};
use coordinates::{ChunkIndex, SuperchunkIndex};

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
    // Hot bitmaps, buckets kept, and the buckets' bytes.
    let held_counts = |arena_measured: &BitmapArena| {
        let buckets: usize = arena_measured.layers().map(|layer| layer.chunks_with_bucket.count_ones() as usize).sum();
        let bucket_bytes: u64 = arena_measured.layers().map(|layer| size_of_val(&*layer.buckets) as u64).sum();
        (arena_measured.len(), buckets, bucket_bytes)
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
