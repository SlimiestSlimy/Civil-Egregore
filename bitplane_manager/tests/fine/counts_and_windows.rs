//! What the arena counts and reads at once: set cells by bitmap and
//! count tile, windows of cells, and wide planes' numbers.
//!
//! `cargo test`

use crate::tests::*;
use bitmap::CellWords;
use bitplane_manager::{BitmapArena, BucketKey, NotHot, Reader, Shape, Window, Write, WriteOp, COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS};
use chunk_storage::{Bits16, Bits2, Bits4, Bits8, Wide, Width, ChunkStorage, LayerCodec};
use utilities::rng::Rng;
use utilities::seed::counted;
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};

/// The mock superchunk, made hot: every cell is dirt or grass, never
/// both; each bitmap counts its set cells, a full chunk's 65,536
/// included, and each superchunk bitplane the cells of its hot bitmaps.
#[test]
fn every_bitmap_counts_its_cells() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(WORLD_MIDDLE, grass_on_dirt(counted(), 8, &mut codec));
    for chunk in WORLD_MIDDLE.chunks() {
        arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &storage, &mut codec);
    }
    let (mut full_chunks, mut grass) = (0, 0);
    for chunk in WORLD_MIDDLE.chunks() {
        let (dirt_bucket, grass_bucket) = (
            arena.bucket(BucketKey { layer_type: MOCK_DIRT, chunk }).expect("hot"),
            arena.bucket(BucketKey { layer_type: MOCK_GRASS, chunk }).expect("hot"),
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
    assert_eq!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), grass);
    assert_eq!(arena.superchunk_count(MOCK_DIRT, WORLD_MIDDLE), (1 << 20) - grass);
    assert_eq!(arena.superchunk_count(MOCK_GRASS, ORIGIN), 0, "nothing hot there");
}

/// Counts move by one a cell changed, not at all for a cell already so;
/// an evicted bitmap's cells leave its superchunk's count, and come back
/// with it.
#[test]
fn counts_follow_every_change() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    let mut random = Rng::new(counted());
    storage.insert(WORLD_MIDDLE, grass_on_dirt(counted(), 0, &mut codec));
    let chunk = ChunkIndex::of(WORLD_MIDDLE, random.below(16) as usize);
    arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &storage, &mut codec);
    let (dirt, grass) = (BucketKey { layer_type: MOCK_DIRT, chunk }, BucketKey { layer_type: MOCK_GRASS, chunk });
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), (1 << 16, 0));

    let cell = cell_in(chunk, (random.below(256) as u8, random.below(256) as u8));
    for _ in 0..2 {
        write(&mut arena, MOCK_GRASS, WriteOp::Set, cell);
        write(&mut arena, MOCK_DIRT, WriteOp::Unset, cell);
    }
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), ((1 << 16) - 1, 1));
    assert_eq!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 1);
    write(&mut arena, MOCK_GRASS, WriteOp::Unset, cell);
    write(&mut arena, MOCK_DIRT, WriteOp::Set, cell);
    assert_eq!((arena.bucket(dirt).expect("hot").count(), arena.bucket(grass).expect("hot").count()), (1 << 16, 0));

    write(&mut arena, MOCK_GRASS, WriteOp::Set, cell);
    arena.write_back(WORLD_MIDDLE, &mut storage, &mut codec);
    assert!(arena.evict(grass));
    assert_eq!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 0, "evicted, waiting in the ring");
    arena.make_hot(grass, None, &mut codec);
    assert_eq!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 1, "back as it was");
}

/// A window of cells read at once is its cells read one by one: at any
/// cell, any size up to 8x8, inside a tile, across tiles, chunks and
/// superchunks, at the world's corner, where some cells are not hot --
/// with superchunks and types read by turns.
#[test]
fn windows_read_at_once_are_the_cells_read_one_by_one() {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    let mut random = Rng::new(counted());
    for y in 0..5 {
        for x in 0..5 {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                // Some chunks of grass left cold, and dirt over half the superchunks.
                if random.below(7) != 0 {
                    arena.make_hot(BucketKey { layer_type: MOCK_GRASS, chunk }, None, &mut codec);
                }
                if (x + y).is_multiple_of(2) {
                    arena.make_hot(BucketKey { layer_type: MOCK_DIRT, chunk }, None, &mut codec);
                }
            }
        }
    }
    let side = 5 * SUPERCHUNK_SIDE_CELLS;
    for _ in 0..40_000 {
        // Half scattered, half crowded about one place: windows with one cell set, and with many.
        let spread = if random.below(2) == 0 { side } else { 64 };
        let cell = CellCartesian { x: random.below(u64::from(spread)) as u32, y: random.below(u64::from(spread)) as u32 };
        arena.queue(if random.below(3) == 0 { MOCK_DIRT } else { MOCK_GRASS }, Write::cell(cell.into(), WriteOp::Set));
    }
    arena.apply();
    let reader = Reader::new(arena.superchunks());
    let edge = SUPERCHUNK_SIDE_CELLS;
    let mut origins = vec![(0, 0), (1, 1), (7, 7), (255, 255), (252, 3), (edge - 1, edge - 1), (edge - 4, edge - 5), (2 * edge, 3 * edge - 1), (side - 8, side - 8)];
    origins.extend((0..3000).map(|at| {
        let reach = u64::from(if at % 3 == 0 { 64 } else { side - 8 });
        (random.below(reach) as u32, random.below(reach) as u32)
    }));
    for (x, y) in origins {
        let (width, height) = (random.between(1, 8) as u32, random.between(1, 8) as u32);
        let origin = CellIndex::from(CellCartesian { x, y });
        for layer_type in [MOCK_GRASS, MOCK_DIRT] {
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
    let mut random = Rng::new(counted());
    storage.insert(WORLD_MIDDLE, grass_on_dirt(counted(), 300_000, &mut codec));
    for chunk in WORLD_MIDDLE.chunks() {
        arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &storage, &mut codec);
    }
    let counted = |arena: &BitmapArena| {
        let superchunk = arena.superchunks().iter().find(|superchunk| superchunk.index() == WORLD_MIDDLE).expect("in use");
        for layer_type in [MOCK_DIRT, MOCK_GRASS] {
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
    for _ in 0..3000 {
        let cell = CellCartesian { x: corner.x + random.below(1000) as u32, y: corner.y + random.below(1000) as u32 };
        let op = [WriteOp::Set, WriteOp::Unset, WriteOp::Flip][random.below(3) as usize];
        let shape = match random.below(50) {
            0 => Shape::Rect { width: random.between(1, 40) as u8, height: random.between(1, 40) as u8 },
            1 => Shape::Disc { radius: random.below(16) as u8 },
            _ => Shape::Cell,
        };
        arena.queue(if random.below(2) == 0 { MOCK_GRASS } else { MOCK_DIRT }, Write { at: cell.into(), op, shape });
    }
    assert!(arena.apply().changed > 1000);
    counted(&arena);
}

/// A wide plane of `W`'s bits a cell holds a number a cell: written,
/// read back whole, counted where it is not 0; written back, it is
/// cold as its bits' bitmaps, a layer type each, and made hot again
/// from them it holds what it held.
fn a_wide_plane_holds_numbers<W: Width>(plane: Wide<W>) {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut storage = ChunkStorage::new(1 << 12);
    let mut random = Rng::new(counted());
    let (layer_type, chunk) = (plane.layer_type(), ChunkIndex::of(WORLD_MIDDLE, random.below(16) as usize));
    let key = BucketKey { layer_type, chunk };
    assert_eq!(arena.make_hot_layers(chunk, &[layer_type], &storage, &mut codec), 1);
    // Numbers over the plane's whole range, on cells of their own; one of them put again, one taken back to 0.
    let mut cells = std::collections::BTreeSet::new();
    while cells.len() < 40 {
        cells.insert(random.below(1 << 16) as usize);
    }
    let numbers: Vec<(CellIndex, u32)> = cells.into_iter().map(|place| (CellIndex::of(chunk, place), 1 + random.below(u64::from(plane.most())) as u32)).collect();
    let (emptied, _) = numbers[7];
    for &(cell, number) in &numbers {
        arena.queue(layer_type, Write::value(plane, cell, number));
    }
    arena.queue(layer_type, Write::value(plane, numbers[0].0, plane.most()));
    arena.queue(layer_type, Write::value(plane, emptied, 0));
    arena.apply();
    let held = |arena: &BitmapArena| numbers.iter().map(|&(cell, _)| arena.value(plane, cell)).collect::<Vec<_>>();
    let expected: Vec<_> = numbers.iter().enumerate().map(|(nth, &(_, number))| Ok(if nth == 0 { plane.most() } else if nth == 7 { 0 } else { number })).collect();
    assert_eq!(held(&arena), expected, "{} bits a cell", W::BITS);
    assert_eq!(arena.bucket(key).expect("hot").count(), 39, "the cells not at 0");

    assert_eq!(arena.write_back(WORLD_MIDDLE, &mut storage, &mut codec), 1);
    storage.flush_all(&mut flushed);
    arena.flushed(&flushed);
    assert!(layer_type.planes().any(|bit| storage.layer(chunk, bit).is_some()) && storage.layer(chunk, layer_type).is_none(), "cold as its bits' bitmaps");
    assert!(arena.evict(key));
    assert_eq!(arena.value(plane, emptied), Err(NotHot(key)));
    assert_eq!(arena.make_hot_layers(chunk, &[layer_type], &storage, &mut codec), 1);
    assert_eq!(held(&arena), expected, "{} bits a cell, from the cold pool", W::BITS);
    assert_eq!(arena.bucket(key).expect("hot").count(), 39);
}

/// Planes of 2, 4, 8 and 16 bits a cell each hold their numbers, hot and cold.
#[test]
fn wide_planes_hold_numbers_at_every_width() {
    a_wide_plane_holds_numbers(Wide::<Bits2>::new(40));
    a_wide_plane_holds_numbers(Wide::<Bits4>::new(40));
    a_wide_plane_holds_numbers(Wide::<Bits8>::new(40));
    a_wide_plane_holds_numbers(Wide::<Bits16>::new(40));
}
