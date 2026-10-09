//! What the arena counts and reads at once: set cells by bitmap and
//! count tile, windows of cells, and wide planes' numbers.
//!
//! `cargo test`

use crate::tests::*;
use bitmap::CellWords;
use bitplane_manager::{BitmapArena, BucketKey, NotHot, Reader, Shape, Window, Write, WriteOp, COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS};
use chunk_storage::mock::{grass_on_dirt, DIRT, GRASS};
use chunk_storage::{Bits16, Bits2, Bits4, Bits8, Wide, Width, ChunkStorage, LayerCodec};
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};

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

/// A wide plane of `W`'s bits a cell holds a number a cell: written,
/// read back whole, counted where it is not 0; written back, it is
/// cold as its bits' bitmaps, a layer type each, and made hot again
/// from them it holds what it held.
fn a_wide_plane_holds_numbers<W: Width>(plane: Wide<W>) {
    let (mut codec, mut arena, mut flushed) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut storage = ChunkStorage::new(1 << 12);
    let (layer_type, chunk) = (plane.layer_type(), ChunkIndex::of(WORLD_MIDDLE, 9));
    let key = BucketKey { layer_type, chunk };
    assert_eq!(arena.make_hot_layers(chunk, &[layer_type], &storage, &mut codec), 1);
    // Numbers over the plane's whole range, on cells of their own; one of them put again, one taken back to 0.
    let numbers: Vec<(CellIndex, u32)> = (0..40).map(|nth: u32| (cell_in(chunk, ((nth * 37) as u8, (nth / 4 * 91) as u8)), 1 + nth.wrapping_mul(2_654_435_761) % plane.most())).collect();
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
