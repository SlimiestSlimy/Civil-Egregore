//! What the arena counts and reads at once: set cells by bitmap and
//! count tile, windows of cells, and wide planes' numbers.
//!
//! `cargo test`

use crate::chunks::*;
use utilities::bitmap::CellWords;
use bitplane_manager::{BitmapArena, BucketKey, NotHot, CellReader, WriteShape, Window, Write, WriteOp, COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS};
use chunk_storage::{Bits16, Bits2, Bits4, Bits8, WidePlane, Width, ChunkStorage, LayerCodec};
use utilities::rng::RandomSource;
use utilities::coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};

/// The mock superchunk, made hot: every cell is dirt or grass, never
/// both; each bitmap counts its set cells, a full chunk's 65,536
/// included, and each superchunk bitplane the cells of its hot bitmaps.
#[test]
fn every_bitmap_counts_its_cells() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut layer_codec = LayerCodec::new();
    let mut bitmap_arena = BitmapArena::new();
    let mut cold_storage = ChunkStorage::new(1 << 12);
    cold_storage.insert(WORLD_MIDDLE, grass_on_dirt(seed, 8, &mut layer_codec));
    for chunk in WORLD_MIDDLE.chunks() {
        bitmap_arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &cold_storage, &mut layer_codec);
    }
    let (mut full_chunks, mut grass_count) = (0, 0);
    for chunk in WORLD_MIDDLE.chunks() {
        let (dirt_bucket, grass_bucket) = (
            bitmap_arena.bucket(BucketKey { layer_type: MOCK_DIRT, chunk }).expect("hot"),
            bitmap_arena.bucket(BucketKey { layer_type: MOCK_GRASS, chunk }).expect("hot"),
        );
        let cells_set_in = |cells: &CellWords| cells.iter().map(|word| word.count_ones()).sum::<u32>();
        assert_eq!(dirt_bucket.count(), cells_set_in(dirt_bucket.cells()));
        assert_eq!(grass_bucket.count(), cells_set_in(grass_bucket.cells()));
        assert_eq!(dirt_bucket.count() + grass_bucket.count(), 1 << 16, "{chunk:?}");
        assert!(dirt_bucket.cells().iter().zip(grass_bucket.cells()).all(|(dirt, grass_word)| dirt & grass_word == 0), "never both");
        full_chunks += (dirt_bucket.count() == 1 << 16) as u32;
        grass_count += grass_bucket.count();
    }
    assert!(full_chunks > 0, "a chunk with no grass: 65,536 cells of dirt");
    assert!((6..=8).contains(&grass_count), "about 8 cells of grass, {grass_count}");
    assert_eq!(bitmap_arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), grass_count);
    assert_eq!(bitmap_arena.superchunk_count(MOCK_DIRT, WORLD_MIDDLE), (1 << 20) - grass_count);
    assert_eq!(bitmap_arena.superchunk_count(MOCK_GRASS, ORIGIN), 0, "nothing hot there");
}

/// Counts move by one a cell changed, not at all for a cell already so;
/// an evicted bitmap's cells leave its superchunk's count, and come back
/// with it.
#[test]
fn counts_follow_every_change() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut layer_codec = LayerCodec::new();
    let mut bitmap_arena = BitmapArena::new();
    let mut cold_storage = ChunkStorage::new(1 << 12);
    let mut seeded_random = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    cold_storage.insert(WORLD_MIDDLE, grass_on_dirt(seed, 0, &mut layer_codec));
    let chunk = ChunkIndex::from_superchunk_place(WORLD_MIDDLE, seeded_random.draw_below(16) as usize);
    bitmap_arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &cold_storage, &mut layer_codec);
    let (dirt, grass_key) = (BucketKey { layer_type: MOCK_DIRT, chunk }, BucketKey { layer_type: MOCK_GRASS, chunk });
    assert_eq!((bitmap_arena.bucket(dirt).expect("hot").count(), bitmap_arena.bucket(grass_key).expect("hot").count()), (1 << 16, 0));

    let cell = cell_in(chunk, (seeded_random.draw_below(256) as u8, seeded_random.draw_below(256) as u8));
    for _ in 0..2 {
        write(&mut bitmap_arena, MOCK_GRASS, WriteOp::SetCell, cell);
        write(&mut bitmap_arena, MOCK_DIRT, WriteOp::ClearCell, cell);
    }
    assert_eq!((bitmap_arena.bucket(dirt).expect("hot").count(), bitmap_arena.bucket(grass_key).expect("hot").count()), ((1 << 16) - 1, 1));
    assert_eq!(bitmap_arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 1);
    write(&mut bitmap_arena, MOCK_GRASS, WriteOp::ClearCell, cell);
    write(&mut bitmap_arena, MOCK_DIRT, WriteOp::SetCell, cell);
    assert_eq!((bitmap_arena.bucket(dirt).expect("hot").count(), bitmap_arena.bucket(grass_key).expect("hot").count()), (1 << 16, 0));

    write(&mut bitmap_arena, MOCK_GRASS, WriteOp::SetCell, cell);
    bitmap_arena.write_back(WORLD_MIDDLE, &mut cold_storage, &mut layer_codec);
    assert!(bitmap_arena.evict(grass_key));
    assert_eq!(bitmap_arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 0, "evicted, waiting in the ring");
    bitmap_arena.make_hot(grass_key, None, &mut layer_codec);
    assert_eq!(bitmap_arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE), 1, "back as it was");
}

/// A window of cells read at once is its cells read one by one: at any
/// cell, any size up to 8x8, inside a tile, across tiles, chunks and
/// superchunks, at the world's corner, where some cells are not hot --
/// with superchunks and types read by turns.
#[test]
fn windows_read_at_once_are_the_cells_read_one_by_one() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    let mut seeded_random = RandomSource::new(seed);
    for y in 0..5 {
        for x in 0..5 {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                // Some chunks of grass left cold, and dirt over half the superchunks.
                if seeded_random.draw_below(7) != 0 {
                    bitmap_arena.make_hot(BucketKey { layer_type: MOCK_GRASS, chunk }, None, &mut layer_codec);
                }
                if (x + y).is_multiple_of(2) {
                    bitmap_arena.make_hot(BucketKey { layer_type: MOCK_DIRT, chunk }, None, &mut layer_codec);
                }
            }
        }
    }
    let side_in_cells = 5 * SUPERCHUNK_SIDE_CELLS;
    for _ in 0..40_000 {
        // Half scattered, half crowded about one place: windows with one cell set, and with many.
        let spread_in_cells = if seeded_random.draw_below(2) == 0 { side_in_cells } else { 64 };
        let cell = CellCartesian { x: seeded_random.draw_below(u64::from(spread_in_cells)) as u32, y: seeded_random.draw_below(u64::from(spread_in_cells)) as u32 };
        bitmap_arena.queue_write(if seeded_random.draw_below(3) == 0 { MOCK_DIRT } else { MOCK_GRASS }, Write::cell(cell.into(), WriteOp::SetCell));
    }
    bitmap_arena.apply();
    let cell_reader = CellReader::new(bitmap_arena.superchunks());
    let superchunk_edge = SUPERCHUNK_SIDE_CELLS;
    let mut origins = vec![(0, 0), (1, 1), (7, 7), (255, 255), (252, 3), (superchunk_edge - 1, superchunk_edge - 1), (superchunk_edge - 4, superchunk_edge - 5), (2 * superchunk_edge, 3 * superchunk_edge - 1), (side_in_cells - 8, side_in_cells - 8)];
    origins.extend((0..3000).map(|drawn_index| {
        let reach = u64::from(if drawn_index % 3 == 0 { 64 } else { side_in_cells - 8 });
        (seeded_random.draw_below(reach) as u32, seeded_random.draw_below(reach) as u32)
    }));
    for (x, y) in origins {
        let (width, height) = (seeded_random.draw_between(1, 8) as u32, seeded_random.draw_between(1, 8) as u32);
        let origin = CellIndex::from(CellCartesian { x, y });
        for layer_type in [MOCK_GRASS, MOCK_DIRT] {
            let mut expected_window = Window::default();
            for (dx, dy) in (0..height).flat_map(|dy| (0..width).map(move |dx| (dx, dy))) {
                if let Ok(cell_is_set) = cell_reader.layer_holds_cell(layer_type, CellIndex::from(CellCartesian { x: x + dx, y: y + dy })) {
                    expected_window.hot |= 1 << (dy * 8 + dx);
                    expected_window.cells_set |= (cell_is_set as u64) << (dy * 8 + dx);
                }
            }
            assert_eq!(cell_reader.window(layer_type, origin, width, height), expected_window, "{width}x{height} at ({x}, {y}), {layer_type:?}");
        }
    }
}

/// Every count tile of a bitmap counts its set cells, as decoded and through
/// every write after -- cells, rectangles and discs, set, cleared and
/// flipped: what sampling passes over a bitmap by.
#[test]
fn every_count_tile_counts_its_cells() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut layer_codec = LayerCodec::new();
    let mut bitmap_arena = BitmapArena::new();
    let mut cold_storage = ChunkStorage::new(1 << 12);
    let mut seeded_random = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    cold_storage.insert(WORLD_MIDDLE, grass_on_dirt(seed, 300_000, &mut layer_codec));
    for chunk in WORLD_MIDDLE.chunks() {
        bitmap_arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &cold_storage, &mut layer_codec);
    }
    let check_counts = |counted_arena: &BitmapArena| {
        let superchunk = counted_arena.superchunks().iter().find(|superchunk| superchunk.superchunk_index() == WORLD_MIDDLE).expect("in use");
        for layer_type in [MOCK_DIRT, MOCK_GRASS] {
            let layer = superchunk.layer(layer_type).expect("hot");
            for chunk in 0..16 {
                let (cells, tile_counts) = (layer.cells(chunk), layer.tile_counts(chunk));
                for tile in 0..COUNT_TILES_IN_CHUNK {
                    let cells_set_counted: u32 = cells[tile * COUNT_TILE_WORDS..][..COUNT_TILE_WORDS].iter().map(|word| word.count_ones()).sum();
                    assert_eq!(tile_counts[tile] as u32, cells_set_counted, "{layer_type:?}, chunk {chunk}, count tile {tile}");
                }
                assert_eq!(tile_counts.iter().map(|&count| count as u32).sum::<u32>(), layer.count(chunk));
            }
        }
    };
    check_counts(&bitmap_arena);
    let superchunk_corner = WORLD_MIDDLE.top_left().cartesian();
    for _ in 0..3000 {
        let cell = CellCartesian { x: superchunk_corner.x + seeded_random.draw_below(1000) as u32, y: superchunk_corner.y + seeded_random.draw_below(1000) as u32 };
        let write_op = [WriteOp::SetCell, WriteOp::ClearCell, WriteOp::FlipCell][seeded_random.draw_below(3) as usize];
        let write_shape = match seeded_random.draw_below(50) {
            0 => WriteShape::RectangleOfCells { width: seeded_random.draw_between(1, 40) as u8, height: seeded_random.draw_between(1, 40) as u8 },
            1 => WriteShape::DiscOfCells { radius_in_cells: seeded_random.draw_below(16) as u8 },
            _ => WriteShape::Cell,
        };
        bitmap_arena.queue_write(if seeded_random.draw_below(2) == 0 { MOCK_GRASS } else { MOCK_DIRT }, Write { anchor_cell: cell.into(), write_op, write_shape });
    }
    assert!(bitmap_arena.apply().cells_changed > 1000);
    check_counts(&bitmap_arena);
}

/// A wide plane of `W`'s bits a cell holds a number a cell: written,
/// read back whole, counted where it is not 0; written back, it is
/// cold as its bits' bitmaps, a layer type each, and made hot again
/// from them it holds what it held.
fn a_wide_plane_holds_numbers<W: Width>(wide_plane: WidePlane<W>) {
    let (mut layer_codec, mut bitmap_arena, mut flushed_superchunks) = (LayerCodec::new(), BitmapArena::new(), Vec::new());
    let mut cold_storage = ChunkStorage::new(1 << 12);
    let mut seeded_random = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (layer_type, chunk) = (wide_plane.layer_type(), ChunkIndex::from_superchunk_place(WORLD_MIDDLE, seeded_random.draw_below(16) as usize));
    let bucket_key = BucketKey { layer_type, chunk };
    assert_eq!(bitmap_arena.make_hot_layers(chunk, &[layer_type], &cold_storage, &mut layer_codec), 1);
    // Numbers over the plane's whole range, on cells of their own; one of them put again, one taken back to 0.
    // Of a plane of tiles, on tiles of their own: a cell drawn in each, and another of the same tile to read it by.
    let tile_shift_bits = layer_type.tile_shift();
    let mut tiles = std::collections::BTreeSet::new();
    while tiles.len() < 40 {
        tiles.insert(seeded_random.draw_below(1 << (16 - tile_shift_bits)) as usize);
    }
    let in_tile = |tile: usize, random_for_tile: &mut RandomSource| CellIndex::from_chunk_place(chunk, tile << tile_shift_bits | random_for_tile.draw_below(1 << tile_shift_bits) as usize);
    let cell_numbers: Vec<(CellIndex, u32)> = tiles.iter().map(|&tile| (in_tile(tile, &mut seeded_random), 1 + seeded_random.draw_below(u64::from(wide_plane.most_value())) as u32)).collect();
    let same_tiles: Vec<CellIndex> = tiles.iter().map(|&tile| in_tile(tile, &mut seeded_random)).collect();
    let (emptied_cell, _) = cell_numbers[7];
    for &(cell, cell_number) in &cell_numbers {
        bitmap_arena.queue_write(layer_type, Write::value(wide_plane, cell, cell_number));
    }
    bitmap_arena.queue_write(layer_type, Write::value(wide_plane, cell_numbers[0].0, wide_plane.most_value()));
    bitmap_arena.queue_write(layer_type, Write::value(wide_plane, emptied_cell, 0));
    bitmap_arena.apply();
    let numbers_held = |arena_read: &BitmapArena| cell_numbers.iter().map(|&(cell, _)| arena_read.value(wide_plane, cell)).collect::<Vec<_>>();
    let expected_numbers: Vec<_> = cell_numbers.iter().enumerate().map(|(number_index, &(_, written_number))| Ok(if number_index == 0 { wide_plane.most_value() } else if number_index == 7 { 0 } else { written_number })).collect();
    assert_eq!(numbers_held(&bitmap_arena), expected_numbers, "{} bits a cell", W::BITS_A_CELL);
    assert_eq!(same_tiles.iter().map(|&cell| bitmap_arena.value(wide_plane, cell)).collect::<Vec<_>>(), expected_numbers, "every cell of a tile reads the tile's number");
    assert_eq!(bitmap_arena.bucket(bucket_key).expect("hot").count(), 39, "the cells not at 0");
    assert_eq!(bitmap_arena.bucket(bucket_key).expect("hot").words().len(), (1024 * W::BITS_A_CELL as usize) >> tile_shift_bits, "a bitmap's words a bit, over a tile's cells");

    assert_eq!(bitmap_arena.write_back(WORLD_MIDDLE, &mut cold_storage, &mut layer_codec), 1);
    cold_storage.flush_all(&mut flushed_superchunks);
    bitmap_arena.flushed_from_ring(&flushed_superchunks);
    assert!(layer_type.each_bitplane().any(|bit_layer_type| cold_storage.layer(chunk, bit_layer_type).is_some()) && cold_storage.layer(chunk, layer_type).is_none(), "cold as its bits' bitmaps");
    assert!(bitmap_arena.evict(bucket_key));
    assert_eq!(bitmap_arena.value(wide_plane, emptied_cell), Err(NotHot(bucket_key)));
    assert_eq!(bitmap_arena.make_hot_layers(chunk, &[layer_type], &cold_storage, &mut layer_codec), 1);
    assert_eq!(numbers_held(&bitmap_arena), expected_numbers, "{} bits a cell, from the cold pool", W::BITS_A_CELL);
    assert_eq!(bitmap_arena.bucket(bucket_key).expect("hot").count(), 39);
}

/// A plane of tiles 8 by 8 cells holds a number a tile, read at any
/// cell of the tile, in a bucket a sixty-fourth of a plane of cells';
/// hot and cold, as any wide plane.
#[test]
fn planes_of_tiles_hold_a_number_a_tile() {
    a_wide_plane_holds_numbers(WidePlane::<Bits4>::of_tiles(40, 3));
    a_wide_plane_holds_numbers(WidePlane::<Bits16>::of_tiles(40, 3));
}

/// Planes of 2, 4, 8 and 16 bits a cell each hold their numbers, hot and cold.
#[test]
fn wide_planes_hold_numbers_at_every_width() {
    a_wide_plane_holds_numbers(WidePlane::<Bits2>::new(40));
    a_wide_plane_holds_numbers(WidePlane::<Bits4>::new(40));
    a_wide_plane_holds_numbers(WidePlane::<Bits8>::new(40));
    a_wide_plane_holds_numbers(WidePlane::<Bits16>::new(40));
}
