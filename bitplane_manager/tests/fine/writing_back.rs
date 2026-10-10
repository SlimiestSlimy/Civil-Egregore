//! Changes written back into chunk storage's ring, bitmaps evicted and
//! superchunks lingering until the ring is flushed.
//!
//! `cargo test`

use crate::tests::*;
use bitmap::{CellWords, WORDS};
use bitplane_manager::{BitmapArena, BucketKey, NotHot, WriteOp};
use chunk_storage::{ChunkStorage, LayerCodec, LayerType};
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};

/// Writing back encodes only what changed into the ring, and the cold pool
/// holds it once flushed; a layer left with no cell set leaves its
/// chunk. A changed bitmap cannot be evicted before it is written back.
#[test]
fn changes_write_back_through_the_ring() {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let place = a_place();
    let mut storage = storage_with(ORIGIN, place, &[(LayerType(2), one_cell(a_cell()))], &mut codec);
    let chunk = ChunkIndex::of(ORIGIN, place);
    let (new, cleared) = (BucketKey { layer_type: LayerType(1), chunk }, BucketKey { layer_type: LayerType(2), chunk });

    assert_eq!(arena.make_hot_layers(chunk, &[LayerType(1), LayerType(2)], &storage, &mut codec), 2);
    assert_eq!(arena.write_back(ORIGIN, &mut storage, &mut codec), 0, "nothing changed");

    write(&mut arena, LayerType(1), WriteOp::Set, cell_in(chunk, a_cell()));
    write(&mut arena, LayerType(2), WriteOp::Unset, cell_in(chunk, a_cell()));
    assert_eq!(arena.write_back(ORIGIN, &mut storage, &mut codec), 2);
    assert!(storage.layer(chunk, LayerType(1)).is_none() && storage.layer(chunk, LayerType(2)).is_some(), "in the ring yet");
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert!(storage.layer(chunk, LayerType(2)).is_none(), "an empty layer leaves the chunk");
    let mut back = [0; WORDS];
    codec.decode(storage.layer(chunk, LayerType(1)).expect("the new layer"), &mut back);
    assert_eq!(back, one_cell(a_cell()));

    assert!(arena.evict(new) && arena.evict(cleared) && arena.is_empty());
    assert_eq!(arena.allocations(), 0, "nothing hot, nothing waiting");
    assert!(!arena.evict(new));
}

/// A bitmap written back and evicted waits in its allocation until its
/// superchunk is flushed: made hot again before then, it is the bucket
/// as it was, though the cold pool does not have it yet; after, it is
/// released, and made hot again it is decoded from the cold pool.
#[test]
fn evicted_bitmaps_wait_for_the_ring() {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut storage = ChunkStorage::new(1 << 12);
    let key = BucketKey { layer_type: LayerType(1), chunk: ChunkIndex::of(WORLD_MIDDLE, a_place()) };
    let cell = cell_in(key.chunk, a_cell());
    arena.make_hot(key, storage.layer(key.chunk, key.layer_type), &mut codec);
    write(&mut arena, LayerType(1), WriteOp::Set, cell);
    arena.write_back(WORLD_MIDDLE, &mut storage, &mut codec);
    assert!(arena.evict(key));
    assert_eq!((arena.len(), arena.allocations()), (0, 1), "evicted, waiting");

    assert!(storage.layer(key.chunk, key.layer_type).is_none(), "not in the cold pool yet");
    assert!(arena.make_hot(key, None, &mut codec));
    assert_eq!(arena.holds(LayerType(1), cell), Ok(true), "the bucket as it was");
    assert!(arena.evict(key));

    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert_eq!(arena.allocations(), 0, "released once flushed");
    arena.make_hot(key, storage.layer(key.chunk, key.layer_type), &mut codec);
    assert_eq!(arena.holds(LayerType(1), cell), Ok(true), "decoded from the cold pool");
}

/// A superchunk made cold as a whole hands back its changed bitmaps to
/// encode, and is set aside, lingering: not hot, it is let go once its
/// changes are flushed -- unless held, wanted hot again, when it is made
/// hot as it was; let go, it is decoded from the cold pool.
#[test]
fn a_superchunk_lingering_waits_for_its_changes() {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut storage = ChunkStorage::new(1 << 12);
    let key = BucketKey { layer_type: LayerType(1), chunk: ChunkIndex::of(WORLD_MIDDLE, a_place()) };
    let cell = cell_in(key.chunk, a_cell());
    arena.make_hot(key, None, &mut codec);
    write(&mut arena, LayerType(1), WriteOp::Set, cell);

    let dirty = arena.make_cold_superchunk(WORLD_MIDDLE);
    assert_eq!(dirty.iter().map(|(key, cells)| (*key, cells.to_vec())).collect::<Vec<_>>(), vec![(key, one_cell(a_cell()).to_vec())], "its one change, to encode");
    assert_eq!((arena.len(), arena.lingering()), (0, 1), "cold, lingering");
    assert_eq!(arena.holds(LayerType(1), cell), Err(NotHot(key)));
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert_eq!(arena.lingering(), 1, "its change on its way: kept");

    for (key, cells) in &dirty {
        assert!(storage.try_write_back(key.chunk, key.layer_type, codec.encode_layer(cells[..].try_into().expect("a bitmap's words"))));
    }
    arena.written_back(WORLD_MIDDLE, dirty.iter().map(|(key, _)| *key));
    assert!(arena.hold(WORLD_MIDDLE), "lingering, so held");
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert_eq!(arena.lingering(), 1, "flushed, but held");
    assert!(arena.make_hot_again(WORLD_MIDDLE));
    assert_eq!((arena.lingering(), arena.holds(LayerType(1), cell)), (0, Ok(true)), "hot as it was");

    assert!(arena.make_cold_superchunk(WORLD_MIDDLE).is_empty(), "nothing changed since");
    assert_eq!(arena.lingering(), 0, "nothing on its way or in the ring: let go at once");
    assert!(!arena.make_hot_again(WORLD_MIDDLE));
    arena.make_hot(key, storage.layer(key.chunk, key.layer_type), &mut codec);
    assert_eq!(arena.holds(LayerType(1), cell), Ok(true), "decoded from the cold pool");
}

/// When writing back fills the ring, the superchunk at its tail is
/// flushed, and its evicted bitmaps are released then.
#[test]
fn a_full_ring_releases_what_it_flushed() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let mut storage = ChunkStorage::new(8);
    let (first, second) = (SuperchunkIndex::from_cartesian(7, 7), SuperchunkIndex::from_cartesian(8, 7));
    let key = |superchunk| BucketKey { layer_type: LayerType(3), chunk: ChunkIndex::of(superchunk, 0) };
    // An entry bigger than the ring grows it to the power of two over the
    // entry -- under two entries -- so the second entry flushes the first.
    let encoded = codec.encode(&drawn()).to_vec();
    for superchunk in [first, second] {
        arena.make_hot(key(superchunk), Some(&encoded), &mut codec);
        write(&mut arena, LayerType(3), WriteOp::Set, cell_in(key(superchunk).chunk, a_cell()));
    }
    arena.write_back(first, &mut storage, &mut codec);
    assert!(arena.evict(key(first)));
    assert_eq!(arena.allocations(), 2, "the first waits in the ring");
    arena.write_back(second, &mut storage, &mut codec);
    assert_eq!(arena.allocations(), 1, "flushed to make room, and released");
    assert!(storage.layer(key(first).chunk, LayerType(3)).is_some());
}

#[test]
#[should_panic(expected = "was not written back")]
fn evicting_an_unwritten_change_panics() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let key = BucketKey { layer_type: LayerType(1), chunk: ChunkIndex(0) };
    arena.make_hot(key, None, &mut codec);
    write(&mut arena, LayerType(1), WriteOp::Set, CellIndex(0));
    arena.evict(key);
}

/// Turning a chunk's layers hot by type decodes those types only: the
/// chunk's other layers stay cold.
#[test]
fn only_the_types_asked_for_turn_hot() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let place = a_place();
    let layers: Vec<(LayerType, CellWords)> = [1, 2, 3].map(|layer_type| (LayerType(layer_type), drawn())).to_vec();
    let storage = storage_with(WORLD_MIDDLE, place, &layers, &mut codec);
    let chunk = ChunkIndex::of(WORLD_MIDDLE, place);
    assert_eq!(arena.make_hot_layers(chunk, &[LayerType(3), LayerType(1), LayerType(8)], &storage, &mut codec), 3);
    assert_eq!(arena.make_hot_layers(chunk, &[LayerType(1)], &storage, &mut codec), 0, "already hot");
    let hot: Vec<LayerType> = arena.keys().map(|key| key.layer_type).collect();
    assert_eq!(hot, [LayerType(1), LayerType(3), LayerType(8)]);
    assert_eq!(arena.bucket(BucketKey { layer_type: LayerType(3), chunk }).expect("hot").cells(), &drawn());
    assert!(!arena.is_hot(BucketKey { layer_type: LayerType(2), chunk }));
}
