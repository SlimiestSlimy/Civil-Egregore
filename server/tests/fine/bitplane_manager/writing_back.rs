//! Changes written back into chunk storage's ring, bitmaps evicted and
//! superchunks cooled until the ring is flushed.
//!
//! `cargo test`

use crate::chunks::*;
use crate::test_types::TestTypes;
use utilities::bitmap::{CellWords, WORDS};
use crate::bitplane_manager::{BitmapArena, BucketKey, NotHot, WriteOp};
use crate::chunk_storage::{ChunkStorage, LayerCodec, LayerType};
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};

/// Writing back encodes only what changed into the ring, and the cold pool
/// holds it once flushed; a layer left with no cell set leaves its
/// chunk. A changed bitmap cannot be evicted before it is written back.
#[test]
fn changes_write_back_through_the_ring() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena, mut flushed_superchunks) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let place = a_place();
    let mut cold_storage = storage_with(ORIGIN, place, &[(types.layers[1], one_cell(a_cell()))], &mut layer_codec);
    let chunk = ChunkIndex::from_superchunk_place(ORIGIN, place);
    let (new_key, cleared_key) = (BucketKey { layer_type: types.layers[0], chunk }, BucketKey { layer_type: types.layers[1], chunk });

    assert_eq!(bitmap_arena.make_hot_layers(chunk, &[types.layers[0], types.layers[1]], &cold_storage, &mut layer_codec), 2);
    assert_eq!(bitmap_arena.write_back(ORIGIN, &mut cold_storage, &mut layer_codec), 0, "nothing changed");

    write(&mut bitmap_arena, types.layers[0], WriteOp::SetCell, cell_in(chunk, a_cell()));
    write(&mut bitmap_arena, types.layers[1], WriteOp::ClearCell, cell_in(chunk, a_cell()));
    assert_eq!(bitmap_arena.write_back(ORIGIN, &mut cold_storage, &mut layer_codec), 2);
    assert!(cold_storage.layer(chunk, types.layers[0]).is_none() && cold_storage.layer(chunk, types.layers[1]).is_some(), "in the ring yet");
    cold_storage.flush_all(&mut flushed_superchunks);
    bitmap_arena.flushed_from_ring(&flushed_superchunks);
    assert!(cold_storage.layer(chunk, types.layers[1]).is_none(), "an empty layer leaves the chunk");
    let mut decoded_back = [0; WORDS];
    layer_codec.decode_cells(cold_storage.layer(chunk, types.layers[0]).expect("the new layer"), &mut decoded_back);
    assert_eq!(decoded_back, one_cell(a_cell()));

    assert!(bitmap_arena.evict(new_key) && bitmap_arena.evict(cleared_key) && bitmap_arena.is_empty());
    assert_eq!(bitmap_arena.allocations(), 0, "nothing hot, nothing waiting");
    assert!(!bitmap_arena.evict(new_key));
}

/// A bitmap written back and evicted waits in its allocation until its
/// superchunk is flushed: made hot again before then, it is the bucket
/// as it was, though the cold pool does not have it yet; after, it is
/// released, and made hot again it is decoded from the cold pool.
#[test]
fn evicted_bitmaps_wait_for_the_ring() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena, mut flushed_superchunks) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut cold_storage = ChunkStorage::new(1 << 12);
    let bucket_key = BucketKey { layer_type: types.layers[0], chunk: ChunkIndex::from_superchunk_place(WORLD_MIDDLE, a_place()) };
    let cell = cell_in(bucket_key.chunk, a_cell());
    bitmap_arena.make_hot(bucket_key, cold_storage.layer(bucket_key.chunk, bucket_key.layer_type), &mut layer_codec);
    write(&mut bitmap_arena, types.layers[0], WriteOp::SetCell, cell);
    bitmap_arena.write_back(WORLD_MIDDLE, &mut cold_storage, &mut layer_codec);
    assert!(bitmap_arena.evict(bucket_key));
    assert_eq!((bitmap_arena.len(), bitmap_arena.allocations()), (0, 1), "evicted, waiting");

    assert!(cold_storage.layer(bucket_key.chunk, bucket_key.layer_type).is_none(), "not in the cold pool yet");
    assert!(bitmap_arena.make_hot(bucket_key, None, &mut layer_codec));
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], cell), Ok(true), "the bucket as it was");
    assert!(bitmap_arena.evict(bucket_key));

    cold_storage.flush_all(&mut flushed_superchunks);
    bitmap_arena.flushed_from_ring(&flushed_superchunks);
    assert_eq!(bitmap_arena.allocations(), 0, "released once flushed");
    bitmap_arena.make_hot(bucket_key, cold_storage.layer(bucket_key.chunk, bucket_key.layer_type), &mut layer_codec);
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], cell), Ok(true), "decoded from the cold pool");
}

/// A superchunk made cold as a whole hands back its changed bitmaps to
/// encode, and is set aside, cooled: not hot, it is let go once its
/// changes are flushed -- unless held, wanted hot again, when it is made
/// hot as it was; let go, it is decoded from the cold pool.
#[test]
fn a_cooled_superchunk_waits_for_its_changes() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena, mut flushed_superchunks) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut cold_storage = ChunkStorage::new(1 << 12);
    let bucket_key = BucketKey { layer_type: types.layers[0], chunk: ChunkIndex::from_superchunk_place(WORLD_MIDDLE, a_place()) };
    let cell = cell_in(bucket_key.chunk, a_cell());
    bitmap_arena.make_hot(bucket_key, None, &mut layer_codec);
    write(&mut bitmap_arena, types.layers[0], WriteOp::SetCell, cell);

    let dirty = bitmap_arena.make_cold_superchunk(WORLD_MIDDLE);
    assert_eq!(dirty.iter().map(|(dirty_key, cells)| (*dirty_key, cells.to_vec())).collect::<Vec<_>>(), vec![(bucket_key, one_cell(a_cell()).to_vec())], "its one change, to encode");
    assert_eq!((bitmap_arena.len(), bitmap_arena.cooled_count()), (0, 1), "cold, cooled");
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], cell), Err(NotHot(bucket_key)));
    cold_storage.flush_all(&mut flushed_superchunks);
    bitmap_arena.flushed_from_ring(&flushed_superchunks);
    assert_eq!(bitmap_arena.cooled_count(), 1, "its change on its way: kept");

    for (written_key, cells) in &dirty {
        assert!(cold_storage.try_write_back(written_key.chunk, written_key.layer_type, layer_codec.encode_layer(cells[..].try_into().expect("a bitmap's words"))));
    }
    bitmap_arena.written_back(WORLD_MIDDLE, dirty.iter().map(|(returned_key, _)| *returned_key));
    assert!(bitmap_arena.hold_cooled(WORLD_MIDDLE), "cooled, so held");
    cold_storage.flush_all(&mut flushed_superchunks);
    bitmap_arena.flushed_from_ring(&flushed_superchunks);
    assert_eq!(bitmap_arena.cooled_count(), 1, "flushed, but held");
    assert!(bitmap_arena.make_hot_again(WORLD_MIDDLE));
    assert_eq!((bitmap_arena.cooled_count(), bitmap_arena.layer_holds_cell(types.layers[0], cell)), (0, Ok(true)), "hot as it was");

    assert!(bitmap_arena.make_cold_superchunk(WORLD_MIDDLE).is_empty(), "nothing changed since");
    assert_eq!(bitmap_arena.cooled_count(), 0, "nothing on its way or in the ring: let go at once");
    assert!(!bitmap_arena.make_hot_again(WORLD_MIDDLE));
    bitmap_arena.make_hot(bucket_key, cold_storage.layer(bucket_key.chunk, bucket_key.layer_type), &mut layer_codec);
    assert_eq!(bitmap_arena.layer_holds_cell(types.layers[0], cell), Ok(true), "decoded from the cold pool");
}

/// When writing back fills the ring, the superchunk at its tail is
/// flushed, and its evicted bitmaps are released then.
#[test]
fn a_full_ring_releases_what_it_flushed() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let mut cold_storage = ChunkStorage::new(8);
    let (first_superchunk, second_superchunk) = (SuperchunkIndex::from_cartesian(7, 7), SuperchunkIndex::from_cartesian(8, 7));
    let bucket_key_of = |superchunk| BucketKey { layer_type: types.layers[2], chunk: ChunkIndex::from_superchunk_place(superchunk, 0) };
    // An entry bigger than the ring grows it to the power of two over the
    // entry -- under two entries -- so the second entry flushes the first.
    let encoded_layer = layer_codec.encode_cells(&rectangle_and_circle()).to_vec();
    for superchunk in [first_superchunk, second_superchunk] {
        bitmap_arena.make_hot(bucket_key_of(superchunk), Some(&encoded_layer), &mut layer_codec);
        // A change whichever cell is drawn: set if the bitmap drawn left it clear, cleared if it set it.
        let cell = cell_in(bucket_key_of(superchunk).chunk, a_cell());
        let write_op = if bitmap_arena.layer_holds_cell(types.layers[2], cell) == Ok(true) { WriteOp::ClearCell } else { WriteOp::SetCell };
        write(&mut bitmap_arena, types.layers[2], write_op, cell);
    }
    bitmap_arena.write_back(first_superchunk, &mut cold_storage, &mut layer_codec);
    assert!(bitmap_arena.evict(bucket_key_of(first_superchunk)));
    assert_eq!(bitmap_arena.allocations(), 2, "the first waits in the ring");
    bitmap_arena.write_back(second_superchunk, &mut cold_storage, &mut layer_codec);
    assert_eq!(bitmap_arena.allocations(), 1, "flushed to make room, and released");
    assert!(cold_storage.layer(bucket_key_of(first_superchunk).chunk, types.layers[2]).is_some());
}

#[test]
#[should_panic(expected = "was not written back")]
fn evicting_an_unwritten_change_panics() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let bucket_key = BucketKey { layer_type: types.layers[0], chunk: ChunkIndex(0) };
    bitmap_arena.make_hot(bucket_key, None, &mut layer_codec);
    write(&mut bitmap_arena, types.layers[0], WriteOp::SetCell, CellIndex(0));
    bitmap_arena.evict(bucket_key);
}

/// Turning a chunk's layers hot by type decodes those types only: the
/// chunk's other layers stay cold.
#[test]
fn only_the_types_asked_for_turn_hot() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut layer_codec = LayerCodec::new();
    let mut bitmap_arena = BitmapArena::new();
    let place = a_place();
    let layers: Vec<(LayerType, CellWords)> = [0, 1, 2].map(|place_among| (types.layers[place_among], rectangle_and_circle())).to_vec();
    let cold_storage = storage_with(WORLD_MIDDLE, place, &layers, &mut layer_codec);
    let chunk = ChunkIndex::from_superchunk_place(WORLD_MIDDLE, place);
    assert_eq!(bitmap_arena.make_hot_layers(chunk, &[types.layers[2], types.layers[0], types.layers[3]], &cold_storage, &mut layer_codec), 3);
    assert_eq!(bitmap_arena.make_hot_layers(chunk, &[types.layers[0]], &cold_storage, &mut layer_codec), 0, "already hot");
    let hot: Vec<LayerType> = bitmap_arena.hot_bucket_keys().map(|hot_key| hot_key.layer_type).collect();
    let mut hot_in_type_order = [types.layers[0], types.layers[2], types.layers[3]];
    hot_in_type_order.sort_unstable();
    assert_eq!(hot, hot_in_type_order);
    assert_eq!(bitmap_arena.bucket(BucketKey { layer_type: types.layers[2], chunk }).expect("hot").cells(), &rectangle_and_circle());
    assert!(!bitmap_arena.is_hot(BucketKey { layer_type: types.layers[1], chunk }));
}
