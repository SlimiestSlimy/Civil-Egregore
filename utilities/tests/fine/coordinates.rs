//! Coordinates: chunk and cell indices nested, converted to and from
//! cartesian coordinates, and stepping as cartesian coordinates would.
//!
//! `cargo test`

use utilities::rng::RandomSource;
use utilities::coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, CellIndex, ChunkIndex, CELLS_IN_CHUNK, CHUNK_SIDE_CELLS, WORLD_MIDDLE, WORLD_SIDE_CHUNKS};

/// The cartesian coordinate, each way, of the chunk in the middle
/// of the world.
const MIDDLE_CHUNK_CARTESIAN: u32 = WORLD_SIDE_CHUNKS / 2;

/// The run's random numbers: what a test draws its cases from.
fn seeded_random_numbers() -> RandomSource {
    RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed())
}

/// `x` and `y`'s bits interleaved one at a time, `x` in the even bits:
/// a Morton index the slow way.
fn interleaved_bit_by_bit(x: u32, y: u32) -> u64 {
    (0..u32::BITS).fold(0, |index, bit| index | u64::from(x >> bit & 1) << (2 * bit) | u64::from(y >> bit & 1) << (2 * bit + 1))
}

/// `count` numbers below `exclusive_bound`, drawn.
fn drawn_numbers(random_numbers: &mut RandomSource, count: usize, exclusive_bound: u64) -> Vec<u32> {
    (0..count).map(|_| random_numbers.draw_below(exclusive_bound) as u32).collect()
}

/// Chunks' indices interleave their coordinates, x in the low bit:
/// the first four of a 2x2 block run top left, top right, bottom left,
/// bottom right, at the world's corner and in its middle alike.
#[test]
fn chunks_sort_in_morton_order() {
    let chunk_from_cartesian = |x, y| ChunkIndex::from_cartesian(x, y);
    assert!(chunk_from_cartesian(0, 0) < chunk_from_cartesian(1, 0) && chunk_from_cartesian(1, 0) < chunk_from_cartesian(0, 1) && chunk_from_cartesian(0, 1) < chunk_from_cartesian(1, 1));
    assert!(chunk_from_cartesian(1, 1) < chunk_from_cartesian(2, 0));
    let (x, y) = (MIDDLE_CHUNK_CARTESIAN & !1, MIDDLE_CHUNK_CARTESIAN & !1);
    assert!(chunk_from_cartesian(x, y) < chunk_from_cartesian(x + 1, y) && chunk_from_cartesian(x + 1, y) < chunk_from_cartesian(x, y + 1) && chunk_from_cartesian(x, y + 1) < chunk_from_cartesian(x + 1, y + 1));
    let last_chunk_cartesian = WORLD_SIDE_CHUNKS - 1;
    assert_eq!(chunk_from_cartesian(last_chunk_cartesian, last_chunk_cartesian), ChunkIndex((1 << 44) - 1));
    // Anywhere: the four of an aligned 2x2 block in that order, the next block along after them, and each index back to its coordinates.
    let mut random_numbers = seeded_random_numbers();
    for (x, y) in drawn_numbers(&mut random_numbers, 256, u64::from(WORLD_SIDE_CHUNKS)).into_iter().zip(drawn_numbers(&mut random_numbers, 256, u64::from(WORLD_SIDE_CHUNKS))) {
        assert_eq!(chunk_from_cartesian(x, y).cartesian(), (x, y));
        let (x, y) = (x & !1, y & !1);
        assert!(chunk_from_cartesian(x, y) < chunk_from_cartesian(x + 1, y) && chunk_from_cartesian(x + 1, y) < chunk_from_cartesian(x, y + 1) && chunk_from_cartesian(x, y + 1) < chunk_from_cartesian(x + 1, y + 1), "the block at ({x}, {y})");
        assert_eq!(chunk_from_cartesian(x + 1, y + 1).0, chunk_from_cartesian(x, y).0 + 3, "the block at ({x}, {y})");
    }
}

/// A cell's index is its chunk's, then 20 bits for its place in the
/// chunk -- each bit by bit its cartesian coordinates interleaved -- and
/// the cell comes back from its chunk and place, and from its cartesian
/// coordinates.
#[test]
fn a_cells_index_is_its_chunks_then_its_place() {
    let middle_cell_cartesian = MIDDLE_CHUNK_CARTESIAN * CHUNK_SIDE_CELLS;
    let edge_offsets = [0, 1, CHUNK_SIDE_CELLS - 1, CHUNK_SIDE_CELLS];
    // The edges, each from the world's start, its middle and its end -- and coordinates drawn from all there are.
    let mut tested_coordinates: Vec<u32> = edge_offsets.iter().flat_map(|&edge_offset| [edge_offset, middle_cell_cartesian + edge_offset, middle_cell_cartesian - edge_offset - 1, u32::MAX - edge_offset]).collect();
    tested_coordinates.extend(drawn_numbers(&mut seeded_random_numbers(), 40, 1 << 32));
    for &x in &tested_coordinates {
        for &y in &tested_coordinates {
            let cartesian = CellCartesian { x, y };
            let cell = CellIndex::from(cartesian);
            assert_eq!(cell.0, interleaved_bit_by_bit(x, y), "cell ({x}, {y})");
            assert_eq!(cell.cartesian(), cartesian, "cell ({x}, {y})");
            let chunk = cell.chunk();
            assert_eq!(chunk.cartesian(), (x / CHUNK_SIDE_CELLS, y / CHUNK_SIDE_CELLS), "cell ({x}, {y})");
            assert_eq!(cell.place(), place_from_cartesian(x % CHUNK_SIDE_CELLS, y % CHUNK_SIDE_CELLS), "cell ({x}, {y})");
            assert_eq!(cartesian_from_place(cell.place()), (x % CHUNK_SIDE_CELLS, y % CHUNK_SIDE_CELLS), "cell ({x}, {y})");
            assert_eq!(CellIndex::from_chunk_place(chunk, cell.place()), cell, "cell ({x}, {y})");
            assert_eq!(chunk.top_left(), CellIndex::from_chunk_place(chunk, 0), "cell ({x}, {y})");
        }
    }
    // The cell just up and left of the middle chunk is the last of the chunk up and left of it.
    let last_cell_before_middle = CellIndex::from(CellCartesian { x: middle_cell_cartesian - 1, y: middle_cell_cartesian - 1 });
    assert_eq!((last_cell_before_middle.chunk(), last_cell_before_middle.place()), (ChunkIndex::from_cartesian(MIDDLE_CHUNK_CARTESIAN - 1, MIDDLE_CHUNK_CARTESIAN - 1), CELLS_IN_CHUNK - 1));
    assert_eq!(WORLD_MIDDLE.top_left().cartesian(), CellCartesian { x: middle_cell_cartesian, y: middle_cell_cartesian });
    assert_eq!(CellIndex::from(CellCartesian { x: u32::MAX, y: u32::MAX }), CellIndex(u64::MAX));
}

/// A cell's index steps to its neighbours on the index itself, as its
/// cartesian coordinates would -- refusing to step past the world's
/// edges.
#[test]
fn cell_indices_step_like_cartesian_coordinates() {
    let middle_edge_cartesian = MIDDLE_CHUNK_CARTESIAN * CHUNK_SIDE_CELLS;
    // The world's corners and the borders of chunks, then cells drawn: half anywhere, half within a step of a chunk's border.
    let mut cells = vec![(0, 0), (1, 0), (1023, 1024), (middle_edge_cartesian - 1, middle_edge_cartesian), (middle_edge_cartesian + 1023, middle_edge_cartesian + 1023), (u32::MAX, u32::MAX), (u32::MAX, 0), (0, u32::MAX), (1, 1), (1022, 1022)];
    let mut random_numbers = seeded_random_numbers();
    let mut drawn_coordinate = |near_a_border: bool| {
        let anywhere_coordinate = random_numbers.draw_below(1 << 32) as u32;
        if near_a_border { (anywhere_coordinate & !(CHUNK_SIDE_CELLS - 1)).wrapping_add(random_numbers.draw_below(3) as u32).wrapping_sub(1) } else { anywhere_coordinate }
    };
    cells.extend((0..200).map(|case_number| (drawn_coordinate(case_number % 2 == 0), drawn_coordinate(case_number % 2 == 0))));
    // Every neighbour and no step, then steps drawn: as far as an entity reaches and further.
    let mut steps = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0)];
    steps.extend((0..24).map(|_| (random_numbers.draw_below(4097) as i32 - 2048, random_numbers.draw_below(4097) as i32 - 2048)));
    for (x, y) in cells {
        let cell_index = CellIndex::from(CellCartesian { x, y });
        for &(dx, dy) in &steps {
            let expected_cell = x.checked_add_signed(dx).zip(y.checked_add_signed(dy)).map(|(x, y)| CellIndex::from(CellCartesian { x, y }));
            assert_eq!(cell_index.at_cartesian_offset(dx, dy), expected_cell, "({x}, {y}) by ({dx}, {dy})");
        }
    }
}

/// A chunk's index steps to its neighbours as its cartesian
/// coordinates would, refusing to step past the world's edges.
#[test]
fn chunk_indices_step_like_cartesian_coordinates() {
    let last_chunk_cartesian = WORLD_SIDE_CHUNKS - 1;
    let mut random_numbers = seeded_random_numbers();
    // The world's corners and its middle, then chunks drawn: half anywhere, half on or beside the world's edges.
    let mut chunks = vec![(0, 0), (1, 0), (MIDDLE_CHUNK_CARTESIAN, MIDDLE_CHUNK_CARTESIAN), (MIDDLE_CHUNK_CARTESIAN - 1, MIDDLE_CHUNK_CARTESIAN), (last_chunk_cartesian, last_chunk_cartesian), (last_chunk_cartesian, 0), (0, last_chunk_cartesian)];
    let mut drawn_coordinate = |by_an_edge: bool| match by_an_edge {
        true => [0, 1, last_chunk_cartesian - 1, last_chunk_cartesian][random_numbers.draw_below(4) as usize],
        false => random_numbers.draw_below(u64::from(WORLD_SIDE_CHUNKS)) as u32,
    };
    chunks.extend((0..100).map(|case_number| (drawn_coordinate(case_number % 2 == 0), drawn_coordinate(case_number % 4 < 2))));
    let mut steps = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0)];
    steps.extend((0..16).map(|_| (random_numbers.draw_below(129) as i32 - 64, random_numbers.draw_below(129) as i32 - 64)));
    for (x, y) in chunks {
        let chunk = ChunkIndex::from_cartesian(x, y);
        for &(dx, dy) in &steps {
            let expected_chunk = x
                .checked_add_signed(dx)
                .zip(y.checked_add_signed(dy))
                .filter(|&(x, y)| x < WORLD_SIDE_CHUNKS && y < WORLD_SIDE_CHUNKS)
                .map(|(x, y)| ChunkIndex::from_cartesian(x, y));
            assert_eq!(chunk.at_cartesian_offset(dx, dy), expected_chunk, "({x}, {y}) by ({dx}, {dy})");
        }
    }
}
