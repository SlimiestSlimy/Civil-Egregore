//! The bitplane manager: hot bitmaps decoded from chunk storage's cold
//! pool, read and changed, written back into its ring, evicted -- and
//! never moved.
//!
//! `cargo test`

use bitmap::{Bitmap, CellWords, WORDS};
use bitplane_manager::{WritesApplied, BitmapArena, BucketKey, NotHot, Reader, Shape, Window, Write, WriteOp, COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS};
use chunk_storage::mock::{grass_on_dirt, DIRT, GRASS};
use chunk_storage::{ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use bitmap::morton::morton_index;
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};

/// A cell of a chunk, cartesian: across and down from its top left.
const CELL: (u8, u8) = (3, 200);

/// The superchunk at the world's top left corner.
const ORIGIN: SuperchunkIndex = SuperchunkIndex(0);

/// A bitmap's cells, with a rectangle and a circle drawn.
fn drawn() -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(10, 10, 40, 30);
    bitmap.set_circle(180, 180, 25);
    *bitmap.words()
}

/// Queues `op` on `cell` of `layer_type`'s bitplane, and applies it:
/// what applying did.
fn write(arena: &mut BitmapArena, layer_type: LayerType, op: WriteOp, cell: CellIndex) -> WritesApplied {
    arena.queue(layer_type, Write::cell(cell, op));
    arena.apply()
}

/// The cell `(x, y)` across and down from `chunk`'s top left.
fn cell_in(chunk: ChunkIndex, (x, y): (u8, u8)) -> CellIndex {
    CellIndex::of(chunk, morton_index(x, y))
}

/// A bitmap's cells with only `(x, y)` set.
fn one_cell((x, y): (u8, u8)) -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set(x, y);
    *bitmap.words()
}

/// Chunk storage holding `superchunk`, whose chunk at `place` has
/// `layers`, each with its cells.
fn storage_with(superchunk: SuperchunkIndex, place: usize, layers: &[(LayerType, CellWords)], codec: &mut LayerCodec) -> ChunkStorage {
    let encoded: Vec<(LayerType, Vec<u64>)> = layers.iter().map(|(layer_type, cells)| (*layer_type, codec.encode(cells).to_vec())).collect();
    let changes: Vec<LayerChange> =
        encoded.iter().map(|(layer_type, words)| LayerChange { place, layer_type: *layer_type, encoded: words }).collect();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()).rewritten(&changes));
    storage
}

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

/// A hot bitmap stays where it is while others turn hot and cold, in
/// its superchunk and in new ones; and a superchunk's allocation freed
/// is the next one used.
#[test]
fn hot_bitmaps_never_move() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let key = |layer_type, chunk| BucketKey { layer_type: LayerType(layer_type), chunk: ChunkIndex(chunk) };
    let first = key(5, 15);
    arena.make_hot(first, None, &mut codec);
    let address = |arena: &BitmapArena, key| arena.bucket(key).expect("hot").cells().as_ptr();
    let before = address(&arena, first);
    for layer_type in 0..8 {
        for superchunk in 0..8 {
            arena.make_hot(key(layer_type, superchunk * 16), None, &mut codec);
            arena.make_hot(key(5, 12 + superchunk % 2), None, &mut codec);
        }
    }
    assert_eq!(address(&arena, first), before);

    let lone = key(100, 1 << 30);
    let encoded = codec.encode(&drawn()).to_vec();
    arena.make_hot(lone, Some(&encoded), &mut codec);
    let freed = address(&arena, lone);
    assert!(arena.evict(lone));
    let next = key(101, 1 << 30);
    arena.make_hot(next, None, &mut codec);
    assert_eq!(address(&arena, next), freed, "the freed allocation, reused");
    assert!(arena.bucket(next).expect("hot").cells().iter().all(|&word| word == 0), "and cleared");
}

/// Writing back encodes only what changed into the ring, and the cold pool
/// holds it once flushed; a layer left with no cell set leaves its
/// chunk. A changed bitmap cannot be evicted before it is written back.
#[test]
fn changes_write_back_through_the_ring() {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let place = 14;
    let mut storage = storage_with(ORIGIN, place, &[(LayerType(2), one_cell(CELL))], &mut codec);
    let chunk = ChunkIndex::of(ORIGIN, place);
    let (new, cleared) = (BucketKey { layer_type: LayerType(1), chunk }, BucketKey { layer_type: LayerType(2), chunk });

    assert_eq!(arena.make_hot_layers(chunk, &[LayerType(1), LayerType(2)], &storage, &mut codec), 2);
    assert_eq!(arena.write_back(ORIGIN, &mut storage, &mut codec), 0, "nothing changed");

    write(&mut arena, LayerType(1), WriteOp::Set, cell_in(chunk, CELL));
    write(&mut arena, LayerType(2), WriteOp::Unset, cell_in(chunk, CELL));
    assert_eq!(arena.write_back(ORIGIN, &mut storage, &mut codec), 2);
    assert!(storage.layer(chunk, LayerType(1)).is_none() && storage.layer(chunk, LayerType(2)).is_some(), "in the ring yet");
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert!(storage.layer(chunk, LayerType(2)).is_none(), "an empty layer leaves the chunk");
    let mut back = [0; WORDS];
    codec.decode(storage.layer(chunk, LayerType(1)).expect("the new layer"), &mut back);
    assert_eq!(back, one_cell(CELL));

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
    let key = BucketKey { layer_type: LayerType(1), chunk: ChunkIndex::of(WORLD_MIDDLE, 9) };
    let cell = cell_in(key.chunk, CELL);
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
/// encode, and is set aside, cooling: not hot, it is let go once its
/// changes are flushed -- unless held, wanted hot again, when it is made
/// hot as it was; let go, it is decoded from the cold pool.
#[test]
fn a_superchunk_cooling_waits_for_its_changes() {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut storage = ChunkStorage::new(1 << 12);
    let key = BucketKey { layer_type: LayerType(1), chunk: ChunkIndex::of(WORLD_MIDDLE, 9) };
    let cell = cell_in(key.chunk, CELL);
    arena.make_hot(key, None, &mut codec);
    write(&mut arena, LayerType(1), WriteOp::Set, cell);

    let dirty = arena.make_cold_superchunk(WORLD_MIDDLE);
    assert_eq!(dirty.iter().map(|(key, cells)| (*key, **cells)).collect::<Vec<_>>(), vec![(key, one_cell(CELL))], "its one change, to encode");
    assert_eq!((arena.len(), arena.cooling()), (0, 1), "cold, cooling");
    assert_eq!(arena.holds(LayerType(1), cell), Err(NotHot(key)));
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert_eq!(arena.cooling(), 1, "its change on its way: kept");

    for (key, cells) in &dirty {
        assert!(storage.try_write_back(key.chunk, key.layer_type, codec.encode_layer(cells)));
    }
    arena.written_back(WORLD_MIDDLE, dirty.iter().map(|(key, _)| *key));
    assert!(arena.hold(WORLD_MIDDLE), "cooling, so held");
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert_eq!(arena.cooling(), 1, "flushed, but held");
    assert!(arena.make_hot_again(WORLD_MIDDLE));
    assert_eq!((arena.cooling(), arena.holds(LayerType(1), cell)), (0, Ok(true)), "hot as it was");

    assert!(arena.make_cold_superchunk(WORLD_MIDDLE).is_empty(), "nothing changed since");
    assert_eq!(arena.cooling(), 0, "nothing on its way or in the ring: let go at once");
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
        write(&mut arena, LayerType(3), WriteOp::Set, cell_in(key(superchunk).chunk, CELL));
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
    let place = 13;
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

/// The mock superchunk, made hot: every cell is dirt or grass, never
/// both; each bitmap counts its set cells, a full chunk's 65,536
/// included, and each superchunk bitplane the cells of its hot bitmaps.
#[test]
fn every_bitmap_counts_its_cells() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(WORLD_MIDDLE, grass_on_dirt(7, 8, &mut codec));
    for chunk in WORLD_MIDDLE.chunks() {
        arena.make_hot_layers(chunk, &[DIRT, GRASS], &storage, &mut codec);
    }
    let (mut full_chunks, mut grass) = (0, 0);
    for chunk in WORLD_MIDDLE.chunks() {
        let (dirt_bucket, grass_bucket) = (
            arena.bucket(BucketKey { layer_type: DIRT, chunk }).expect("hot"),
            arena.bucket(BucketKey { layer_type: GRASS, chunk }).expect("hot"),
        );
        let ones = |cells: &CellWords| cells.iter().map(|word| word.count_ones()).sum::<u32>();
        assert_eq!(dirt_bucket.count(), ones(dirt_bucket.cells()));
        assert_eq!(grass_bucket.count(), ones(grass_bucket.cells()));
        assert_eq!(dirt_bucket.count() + grass_bucket.count(), 1 << 16, "{chunk:?}");
        assert!(dirt_bucket.cells().iter().zip(grass_bucket.cells()).all(|(dirt, grass)| dirt & grass == 0), "never both");
        full_chunks += (dirt_bucket.count() == 1 << 16) as u32;
        grass += grass_bucket.count();
    }
    assert!(full_chunks > 0, "a chunk with no grass: 65,536 cells of dirt");
    assert!((6..=8).contains(&grass), "about 8 cells of grass, {grass}");
    assert_eq!(arena.superchunk_count(GRASS, WORLD_MIDDLE), grass);
    assert_eq!(arena.superchunk_count(DIRT, WORLD_MIDDLE), (1 << 20) - grass);
    assert_eq!(arena.superchunk_count(GRASS, ORIGIN), 0, "nothing hot there");
}

/// Counts move by one a cell changed, not at all for a cell already so;
/// an evicted bitmap's cells leave its superchunk's count, and come back
/// with it.
#[test]
fn counts_follow_every_change() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(WORLD_MIDDLE, grass_on_dirt(11, 0, &mut codec));
    let chunk = ChunkIndex::of(WORLD_MIDDLE, 11);
    arena.make_hot_layers(chunk, &[DIRT, GRASS], &storage, &mut codec);
    let (dirt, grass) = (BucketKey { layer_type: DIRT, chunk }, BucketKey { layer_type: GRASS, chunk });
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), (1 << 16, 0));

    let cell = cell_in(chunk, CELL);
    for _ in 0..2 {
        write(&mut arena, GRASS, WriteOp::Set, cell);
        write(&mut arena, DIRT, WriteOp::Unset, cell);
    }
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), ((1 << 16) - 1, 1));
    assert_eq!(arena.superchunk_count(GRASS, WORLD_MIDDLE), 1);
    write(&mut arena, GRASS, WriteOp::Unset, cell);
    write(&mut arena, DIRT, WriteOp::Set, cell);
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), (1 << 16, 0));

    write(&mut arena, GRASS, WriteOp::Set, cell);
    arena.write_back(WORLD_MIDDLE, &mut storage, &mut codec);
    assert!(arena.evict(grass));
    assert_eq!(arena.superchunk_count(GRASS, WORLD_MIDDLE), 0, "evicted, waiting in the ring");
    arena.make_hot(grass, None, &mut codec);
    assert_eq!(arena.superchunk_count(GRASS, WORLD_MIDDLE), 1, "back as it was");
}

/// A window of cells read at once is its cells read one by one: at any
/// cell, any size up to 8x8, inside a tile, across tiles, chunks and
/// superchunks, at the world's corner, where some cells are not hot --
/// with superchunks and types read by turns.
#[test]
fn windows_read_at_once_are_the_cells_read_one_by_one() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 0..5 {
        for x in 0..5 {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                // Some chunks of grass left cold, and dirt over half the superchunks.
                if !(x + y + chunk.place() as u32).is_multiple_of(7) {
                    arena.make_hot(BucketKey { layer_type: GRASS, chunk }, None, &mut codec);
                }
                if (x + y).is_multiple_of(2) {
                    arena.make_hot(BucketKey { layer_type: DIRT, chunk }, None, &mut codec);
                }
            }
        }
    }
    let side = 5 * SUPERCHUNK_SIDE_CELLS;
    for at in 0..40_000u32 {
        let cell = CellCartesian { x: (at * 7919) % side, y: (at * 104_729) % side };
        arena.queue(if at % 3 == 0 { DIRT } else { GRASS }, Write::cell(cell.into(), WriteOp::Set));
    }
    arena.apply();
    let reader = Reader::new(arena.superchunks());
    let edge = SUPERCHUNK_SIDE_CELLS;
    let mut origins = vec![(0, 0), (1, 1), (7, 7), (255, 255), (252, 3), (edge - 1, edge - 1), (edge - 4, edge - 5), (2 * edge, 3 * edge - 1), (side - 8, side - 8)];
    origins.extend((0..3000u32).map(|at| ((at * 31_337) % (side - 8), (at * 7_717) % (side - 8))));
    for (number, (x, y)) in origins.into_iter().enumerate() {
        let (width, height) = (1 + number as u32 % 8, 1 + (number as u32 / 8) % 8);
        let origin = CellIndex::from(CellCartesian { x, y });
        for layer_type in [GRASS, DIRT] {
            let mut expected = Window::default();
            for (dx, dy) in (0..height).flat_map(|dy| (0..width).map(move |dx| (dx, dy))) {
                if let Ok(set) = reader.holds(layer_type, CellIndex::from(CellCartesian { x: x + dx, y: y + dy })) {
                    expected.hot |= 1 << (dy * 8 + dx);
                    expected.set |= (set as u64) << (dy * 8 + dx);
                }
            }
            assert_eq!(reader.window(layer_type, origin, width, height), expected, "{width}x{height} at ({x}, {y}), {layer_type:?}");
        }
    }
}

/// Every count tile of a bitmap counts its set cells, as decoded and through
/// every write after -- cells, rectangles and discs, set, cleared and
/// flipped: what sampling passes over a bitmap by.
#[test]
fn every_count_tile_counts_its_cells() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(WORLD_MIDDLE, grass_on_dirt(7, 300_000, &mut codec));
    for chunk in WORLD_MIDDLE.chunks() {
        arena.make_hot_layers(chunk, &[DIRT, GRASS], &storage, &mut codec);
    }
    let counted = |arena: &BitmapArena| {
        let superchunk = arena.superchunks().iter().find(|superchunk| superchunk.index() == WORLD_MIDDLE).expect("in use");
        for layer_type in [DIRT, GRASS] {
            let layer = superchunk.layer(layer_type).expect("hot");
            for chunk in 0..16 {
                let (cells, tile_counts) = (layer.cells(chunk), layer.tile_counts(chunk));
                for tile in 0..COUNT_TILES_IN_CHUNK {
                    let ones: u32 = cells[tile * COUNT_TILE_WORDS..][..COUNT_TILE_WORDS].iter().map(|word| word.count_ones()).sum();
                    assert_eq!(tile_counts[tile] as u32, ones, "{layer_type:?}, chunk {chunk}, count tile {tile}");
                }
                assert_eq!(tile_counts.iter().map(|&count| count as u32).sum::<u32>(), layer.count(chunk));
            }
        }
    };
    counted(&arena);
    let corner = WORLD_MIDDLE.top_left().cartesian();
    for at in 0..3000u32 {
        let cell = CellCartesian { x: corner.x + (at * 7919) % 1000, y: corner.y + (at * 104_729) % 1000 };
        let op = [WriteOp::Set, WriteOp::Unset, WriteOp::Flip][at as usize % 3];
        let shape = match at % 50 {
            0 => Shape::Rect { width: 20, height: 9 },
            1 => Shape::Disc { radius: 7 },
            _ => Shape::Cell,
        };
        arena.queue(if at % 2 == 0 { GRASS } else { DIRT }, Write { at: cell.into(), op, shape });
    }
    assert!(arena.apply().changed > 1000);
    counted(&arena);
}
