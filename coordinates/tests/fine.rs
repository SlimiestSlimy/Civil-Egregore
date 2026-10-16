//! The fine tier: each test pins one behaviour, at the edges written by hand and at cells drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod coordinates {
    //! Coordinates: superchunk, chunk and cell indices nested, converted to
    //! and from cartesian coordinates, and stepping as cartesian
    //! coordinates would.
    //!
    //! `cargo test`

    use bitmap::morton::morton_index;
    use utilities::rng::RandomSource;
    use coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, CHUNK_SIDE, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};

    /// The cartesian coordinate, each way, of the superchunk in the middle
    /// of the world.
    const MIDDLE_SUPERCHUNK_CARTESIAN: u32 = WORLD_SIDE_SUPERCHUNKS / 2;

    /// The run's random numbers: what a test draws its cases from.
    fn seeded_random_numbers() -> RandomSource {
        RandomSource::new(utilities::seed::counted_seed())
    }

    /// `count` numbers below `exclusive_bound`, drawn.
    fn drawn_numbers(random_numbers: &mut RandomSource, count: usize, exclusive_bound: u64) -> Vec<u32> {
        (0..count).map(|_| random_numbers.draw_below(exclusive_bound) as u32).collect()
    }

    /// A superchunk's chunks go in Morton order, as a bitmap's cells do:
    /// every aligned square of chunks is one run of places.
    #[test]
    fn chunks_in_a_superchunk_go_in_morton_order() {
        let superchunk = WORLD_MIDDLE;
        assert_eq!(superchunk.cartesian(), (MIDDLE_SUPERCHUNK_CARTESIAN, MIDDLE_SUPERCHUNK_CARTESIAN));
        let chunk_side = CHUNK_SIDE as u32;
        let top_left = superchunk.top_left().cartesian();
        let places: Vec<usize> = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (0, 2), (3, 3)]
            .into_iter()
            .map(|(x, y)| CellIndex::from(CellCartesian { x: top_left.x + x * chunk_side, y: top_left.y + y * chunk_side }).chunk().place())
            .collect();
        assert_eq!(places, [0, 1, 2, 3, 4, 8, 15]);
        // Any superchunk's chunks: each at its place, an aligned 2x2 of them one run of four.
        let mut random_numbers = seeded_random_numbers();
        for (x, y) in drawn_numbers(&mut random_numbers, 64, u64::from(WORLD_SIDE_SUPERCHUNKS)).into_iter().zip(drawn_numbers(&mut random_numbers, 64, u64::from(WORLD_SIDE_SUPERCHUNKS))) {
            let superchunk = SuperchunkIndex::from_cartesian(x, y);
            let drawn_superchunk_top_left = superchunk.top_left().cartesian();
            let place = |chunks_across: u32, chunks_down: u32| CellIndex::from(CellCartesian { x: drawn_superchunk_top_left.x + chunks_across * chunk_side, y: drawn_superchunk_top_left.y + chunks_down * chunk_side }).chunk().place();
            let (block_chunks_across, block_chunks_down) = (2 * random_numbers.draw_below(2) as u32, 2 * random_numbers.draw_below(2) as u32);
            let first_chunk_place = place(block_chunks_across, block_chunks_down);
            assert_eq!(first_chunk_place % 4, 0, "superchunk ({x}, {y})");
            assert_eq!([place(block_chunks_across + 1, block_chunks_down), place(block_chunks_across, block_chunks_down + 1), place(block_chunks_across + 1, block_chunks_down + 1)], [first_chunk_place + 1, first_chunk_place + 2, first_chunk_place + 3], "superchunk ({x}, {y})");
            assert!(superchunk.chunks().enumerate().all(|(place, chunk)| chunk.place() == place && chunk.superchunk() == superchunk));
        }
        assert!(superchunk.chunks().enumerate().all(|(place, chunk)| chunk.place() == place && chunk.superchunk() == superchunk));
    }

    /// Superchunks' indices interleave their coordinates, x in the low bit:
    /// the first four of a 2x2 block run top left, top right, bottom left,
    /// bottom right, at the world's corner and in its middle alike.
    #[test]
    fn superchunks_sort_in_morton_order() {
        let superchunk_from_cartesian = |x, y| SuperchunkIndex::from_cartesian(x, y);
        assert!(superchunk_from_cartesian(0, 0) < superchunk_from_cartesian(1, 0) && superchunk_from_cartesian(1, 0) < superchunk_from_cartesian(0, 1) && superchunk_from_cartesian(0, 1) < superchunk_from_cartesian(1, 1));
        assert!(superchunk_from_cartesian(1, 1) < superchunk_from_cartesian(2, 0));
        let (x, y) = (MIDDLE_SUPERCHUNK_CARTESIAN & !1, MIDDLE_SUPERCHUNK_CARTESIAN & !1);
        assert!(superchunk_from_cartesian(x, y) < superchunk_from_cartesian(x + 1, y) && superchunk_from_cartesian(x + 1, y) < superchunk_from_cartesian(x, y + 1) && superchunk_from_cartesian(x, y + 1) < superchunk_from_cartesian(x + 1, y + 1));
        let last_superchunk_cartesian = WORLD_SIDE_SUPERCHUNKS - 1;
        assert_eq!(superchunk_from_cartesian(last_superchunk_cartesian, last_superchunk_cartesian), SuperchunkIndex((1 << 44) - 1));
        // Anywhere: the four of an aligned 2x2 block in that order, the next block along after them, and each index back to its coordinates.
        let mut random_numbers = seeded_random_numbers();
        for (x, y) in drawn_numbers(&mut random_numbers, 256, u64::from(WORLD_SIDE_SUPERCHUNKS)).into_iter().zip(drawn_numbers(&mut random_numbers, 256, u64::from(WORLD_SIDE_SUPERCHUNKS))) {
            assert_eq!(superchunk_from_cartesian(x, y).cartesian(), (x, y));
            let (x, y) = (x & !1, y & !1);
            assert!(superchunk_from_cartesian(x, y) < superchunk_from_cartesian(x + 1, y) && superchunk_from_cartesian(x + 1, y) < superchunk_from_cartesian(x, y + 1) && superchunk_from_cartesian(x, y + 1) < superchunk_from_cartesian(x + 1, y + 1), "the block at ({x}, {y})");
            assert_eq!(superchunk_from_cartesian(x + 1, y + 1).0, superchunk_from_cartesian(x, y).0 + 3, "the block at ({x}, {y})");
        }
    }

    /// A cell's index nests its chunk's and superchunk's: from the lowest
    /// bit, 16 for its place in its chunk, 4 for its chunk's place in its
    /// superchunk, 44 for its superchunk -- and each comes back from its
    /// parts, and the cell from its cartesian coordinates.
    #[test]
    fn a_cells_index_nests_its_chunks_and_superchunks() {
        let middle_cell_cartesian = MIDDLE_SUPERCHUNK_CARTESIAN * SUPERCHUNK_SIDE_CELLS;
        let chunk_side = CHUNK_SIDE as u32;
        let edge_offsets = [0, 1, chunk_side - 1, chunk_side, SUPERCHUNK_SIDE_CELLS - 1, SUPERCHUNK_SIDE_CELLS];
        // The edges, each from the world's start, its middle and its end -- and coordinates drawn from all there are.
        let mut tested_coordinates: Vec<u32> = edge_offsets.iter().flat_map(|&edge_offset| [edge_offset, middle_cell_cartesian + edge_offset, middle_cell_cartesian - edge_offset - 1, u32::MAX - edge_offset]).collect();
        tested_coordinates.extend(drawn_numbers(&mut seeded_random_numbers(), 40, 1 << 32));
        for &x in &tested_coordinates {
            for &y in &tested_coordinates {
                let cartesian = CellCartesian { x, y };
                let cell = CellIndex::from(cartesian);
                assert_eq!(cell.cartesian(), cartesian, "cell ({x}, {y})");
                let (chunk, superchunk) = (cell.chunk(), cell.superchunk());
                assert_eq!(cell.place(), morton_index(x as u8, y as u8), "cell ({x}, {y})");
                assert_eq!(chunk.place(), morton_index((x / chunk_side % 4) as u8, (y / chunk_side % 4) as u8), "cell ({x}, {y})");
                assert_eq!(superchunk.cartesian(), (x / SUPERCHUNK_SIDE_CELLS, y / SUPERCHUNK_SIDE_CELLS), "cell ({x}, {y})");
                assert_eq!(chunk.superchunk(), superchunk);
                assert_eq!(cell.place_in_superchunk(), chunk.place() << 16 | cell.place());
                assert_eq!(cell.place_in_superchunk(), place_from_cartesian(x % SUPERCHUNK_SIDE_CELLS, y % SUPERCHUNK_SIDE_CELLS));
                assert_eq!(cartesian_from_place(cell.place_in_superchunk()), (x % SUPERCHUNK_SIDE_CELLS, y % SUPERCHUNK_SIDE_CELLS));
                assert_eq!(CellIndex::from_chunk_place(ChunkIndex::from_superchunk_place(superchunk, chunk.place()), cell.place()), cell);
                assert_eq!(chunk.top_left(), CellIndex::from_chunk_place(chunk, 0));
                assert_eq!(superchunk.top_left(), CellIndex::from_chunk_place(ChunkIndex::from_superchunk_place(superchunk, 0), 0));
            }
        }
        // The cell just up and left of the middle superchunk is the last of
        // everything in the superchunk up and left of it.
        let last_cell_before_middle = CellIndex::from(CellCartesian { x: middle_cell_cartesian - 1, y: middle_cell_cartesian - 1 });
        assert_eq!((last_cell_before_middle.superchunk(), last_cell_before_middle.chunk().place(), last_cell_before_middle.place()), (SuperchunkIndex::from_cartesian(MIDDLE_SUPERCHUNK_CARTESIAN - 1, MIDDLE_SUPERCHUNK_CARTESIAN - 1), 15, 65535));
        assert_eq!(CellIndex::from(CellCartesian { x: u32::MAX, y: u32::MAX }), CellIndex(u64::MAX));
    }

    /// A cell's index steps to its neighbours on the index itself, as its
    /// cartesian coordinates would -- refusing to step past the world's
    /// edges.
    #[test]
    fn cell_indices_step_like_cartesian_coordinates() {
        let middle_edge_cartesian = MIDDLE_SUPERCHUNK_CARTESIAN * SUPERCHUNK_SIDE_CELLS;
        // The world's corners and the borders of chunks and superchunks, then cells drawn: half anywhere, half within a step of a chunk's border.
        let mut cells = vec![(0, 0), (1, 0), (255, 256), (middle_edge_cartesian - 1, middle_edge_cartesian), (middle_edge_cartesian + 1023, middle_edge_cartesian + 1023), (u32::MAX, u32::MAX), (u32::MAX, 0), (0, u32::MAX), (1, 1), (254, 254)];
        let mut random_numbers = seeded_random_numbers();
        let mut drawn_coordinate = |near_a_border: bool| {
            let anywhere_coordinate = random_numbers.draw_below(1 << 32) as u32;
            if near_a_border { (anywhere_coordinate & !255).wrapping_add(random_numbers.draw_below(3) as u32).wrapping_sub(1) } else { anywhere_coordinate }
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

    /// A superchunk's index steps to its neighbours as its cartesian
    /// coordinates would, refusing to step past the world's edges.
    #[test]
    fn superchunk_indices_step_like_cartesian_coordinates() {
        let last_superchunk_cartesian = WORLD_SIDE_SUPERCHUNKS - 1;
        let mut random_numbers = seeded_random_numbers();
        // The world's corners and its middle, then superchunks drawn: half anywhere, half on or beside the world's edges.
        let mut superchunks = vec![(0, 0), (1, 0), (MIDDLE_SUPERCHUNK_CARTESIAN, MIDDLE_SUPERCHUNK_CARTESIAN), (MIDDLE_SUPERCHUNK_CARTESIAN - 1, MIDDLE_SUPERCHUNK_CARTESIAN), (last_superchunk_cartesian, last_superchunk_cartesian), (last_superchunk_cartesian, 0), (0, last_superchunk_cartesian)];
        let mut drawn_coordinate = |by_an_edge: bool| match by_an_edge {
            true => [0, 1, last_superchunk_cartesian - 1, last_superchunk_cartesian][random_numbers.draw_below(4) as usize],
            false => random_numbers.draw_below(u64::from(WORLD_SIDE_SUPERCHUNKS)) as u32,
        };
        superchunks.extend((0..100).map(|case_number| (drawn_coordinate(case_number % 2 == 0), drawn_coordinate(case_number % 4 < 2))));
        let mut steps = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0)];
        steps.extend((0..16).map(|_| (random_numbers.draw_below(129) as i32 - 64, random_numbers.draw_below(129) as i32 - 64)));
        for (x, y) in superchunks {
            let superchunk = SuperchunkIndex::from_cartesian(x, y);
            for &(dx, dy) in &steps {
                let expected_superchunk = x
                    .checked_add_signed(dx)
                    .zip(y.checked_add_signed(dy))
                    .filter(|&(x, y)| x < WORLD_SIDE_SUPERCHUNKS && y < WORLD_SIDE_SUPERCHUNKS)
                    .map(|(x, y)| SuperchunkIndex::from_cartesian(x, y));
                assert_eq!(superchunk.at_cartesian_offset(dx, dy), expected_superchunk, "({x}, {y}) by ({dx}, {dy})");
            }
        }
    }
}
