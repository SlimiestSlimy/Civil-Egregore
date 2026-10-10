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
    use utilities::rng::Rng;
    use coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, CHUNK_SIDE, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};

    /// The cartesian coordinate, each way, of the superchunk in the middle
    /// of the world.
    const MIDDLE: u32 = WORLD_SIDE_SUPERCHUNKS / 2;

    /// The run's random numbers: what a test draws its cases from.
    fn random() -> Rng {
        Rng::new(utilities::seed::counted())
    }

    /// `count` numbers below `bound`, drawn.
    fn drawn(random: &mut Rng, count: usize, bound: u64) -> Vec<u32> {
        (0..count).map(|_| random.below(bound) as u32).collect()
    }

    /// A superchunk's chunks go in Morton order, as a bitmap's cells do:
    /// every aligned square of chunks is one run of places.
    #[test]
    fn chunks_in_a_superchunk_go_in_morton_order() {
        let superchunk = WORLD_MIDDLE;
        assert_eq!(superchunk.cartesian(), (MIDDLE, MIDDLE));
        let chunk_side = CHUNK_SIDE as u32;
        let top_left = superchunk.top_left().cartesian();
        let places: Vec<usize> = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (0, 2), (3, 3)]
            .into_iter()
            .map(|(x, y)| CellIndex::from(CellCartesian { x: top_left.x + x * chunk_side, y: top_left.y + y * chunk_side }).chunk().place())
            .collect();
        assert_eq!(places, [0, 1, 2, 3, 4, 8, 15]);
        // Any superchunk's chunks: each at its place, an aligned 2x2 of them one run of four.
        let mut random = random();
        for (x, y) in drawn(&mut random, 64, u64::from(WORLD_SIDE_SUPERCHUNKS)).into_iter().zip(drawn(&mut random, 64, u64::from(WORLD_SIDE_SUPERCHUNKS))) {
            let superchunk = SuperchunkIndex::from_cartesian(x, y);
            let corner = superchunk.top_left().cartesian();
            let place = |across: u32, down: u32| CellIndex::from(CellCartesian { x: corner.x + across * chunk_side, y: corner.y + down * chunk_side }).chunk().place();
            let (across, down) = (2 * random.below(2) as u32, 2 * random.below(2) as u32);
            let first = place(across, down);
            assert_eq!(first % 4, 0, "superchunk ({x}, {y})");
            assert_eq!([place(across + 1, down), place(across, down + 1), place(across + 1, down + 1)], [first + 1, first + 2, first + 3], "superchunk ({x}, {y})");
            assert!(superchunk.chunks().enumerate().all(|(place, chunk)| chunk.place() == place && chunk.superchunk() == superchunk));
        }
        assert!(superchunk.chunks().enumerate().all(|(place, chunk)| chunk.place() == place && chunk.superchunk() == superchunk));
    }

    /// Superchunks' indices interleave their coordinates, x in the low bit:
    /// the first four of a 2x2 block run top left, top right, bottom left,
    /// bottom right, at the world's corner and in its middle alike.
    #[test]
    fn superchunks_sort_in_morton_order() {
        let key = |x, y| SuperchunkIndex::from_cartesian(x, y);
        assert!(key(0, 0) < key(1, 0) && key(1, 0) < key(0, 1) && key(0, 1) < key(1, 1));
        assert!(key(1, 1) < key(2, 0));
        let (x, y) = (MIDDLE & !1, MIDDLE & !1);
        assert!(key(x, y) < key(x + 1, y) && key(x + 1, y) < key(x, y + 1) && key(x, y + 1) < key(x + 1, y + 1));
        let last = WORLD_SIDE_SUPERCHUNKS - 1;
        assert_eq!(key(last, last), SuperchunkIndex((1 << 44) - 1));
        // Anywhere: the four of an aligned 2x2 block in that order, the next block along after them, and each index back to its coordinates.
        let mut random = random();
        for (x, y) in drawn(&mut random, 256, u64::from(WORLD_SIDE_SUPERCHUNKS)).into_iter().zip(drawn(&mut random, 256, u64::from(WORLD_SIDE_SUPERCHUNKS))) {
            assert_eq!(key(x, y).cartesian(), (x, y));
            let (x, y) = (x & !1, y & !1);
            assert!(key(x, y) < key(x + 1, y) && key(x + 1, y) < key(x, y + 1) && key(x, y + 1) < key(x + 1, y + 1), "the block at ({x}, {y})");
            assert_eq!(key(x + 1, y + 1).0, key(x, y).0 + 3, "the block at ({x}, {y})");
        }
    }

    /// A cell's index nests its chunk's and superchunk's: from the lowest
    /// bit, 16 for its place in its chunk, 4 for its chunk's place in its
    /// superchunk, 44 for its superchunk -- and each comes back from its
    /// parts, and the cell from its cartesian coordinates.
    #[test]
    fn a_cells_index_nests_its_chunks_and_superchunks() {
        let middle = MIDDLE * SUPERCHUNK_SIDE_CELLS;
        let chunk_side = CHUNK_SIDE as u32;
        let edges = [0, 1, chunk_side - 1, chunk_side, SUPERCHUNK_SIDE_CELLS - 1, SUPERCHUNK_SIDE_CELLS];
        // The edges, each from the world's start, its middle and its end -- and coordinates drawn from all there are.
        let mut coordinates: Vec<u32> = edges.iter().flat_map(|&edge| [edge, middle + edge, middle - edge - 1, u32::MAX - edge]).collect();
        coordinates.extend(drawn(&mut random(), 40, 1 << 32));
        for &x in &coordinates {
            for &y in &coordinates {
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
                assert_eq!(CellIndex::of(ChunkIndex::of(superchunk, chunk.place()), cell.place()), cell);
                assert_eq!(chunk.top_left(), CellIndex::of(chunk, 0));
                assert_eq!(superchunk.top_left(), CellIndex::of(ChunkIndex::of(superchunk, 0), 0));
            }
        }
        // The cell just up and left of the middle superchunk is the last of
        // everything in the superchunk up and left of it.
        let last = CellIndex::from(CellCartesian { x: middle - 1, y: middle - 1 });
        assert_eq!((last.superchunk(), last.chunk().place(), last.place()), (SuperchunkIndex::from_cartesian(MIDDLE - 1, MIDDLE - 1), 15, 65535));
        assert_eq!(CellIndex::from(CellCartesian { x: u32::MAX, y: u32::MAX }), CellIndex(u64::MAX));
    }

    /// A cell's index steps to its neighbours on the index itself, as its
    /// cartesian coordinates would -- refusing to step past the world's
    /// edges.
    #[test]
    fn cell_indices_step_like_cartesian_coordinates() {
        let edge = MIDDLE * SUPERCHUNK_SIDE_CELLS;
        // The world's corners and the borders of chunks and superchunks, then cells drawn: half anywhere, half within a step of a chunk's border.
        let mut cells = vec![(0, 0), (1, 0), (255, 256), (edge - 1, edge), (edge + 1023, edge + 1023), (u32::MAX, u32::MAX), (u32::MAX, 0), (0, u32::MAX), (1, 1), (254, 254)];
        let mut random = random();
        let mut coordinate = |near_a_border: bool| {
            let anywhere = random.below(1 << 32) as u32;
            if near_a_border { (anywhere & !255).wrapping_add(random.below(3) as u32).wrapping_sub(1) } else { anywhere }
        };
        cells.extend((0..200).map(|case| (coordinate(case % 2 == 0), coordinate(case % 2 == 0))));
        // Every neighbour and no step, then steps drawn: as far as an entity reaches and further.
        let mut steps = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0)];
        steps.extend((0..24).map(|_| (random.below(4097) as i32 - 2048, random.below(4097) as i32 - 2048)));
        for (x, y) in cells {
            let index = CellIndex::from(CellCartesian { x, y });
            for &(dx, dy) in &steps {
                let expected = x.checked_add_signed(dx).zip(y.checked_add_signed(dy)).map(|(x, y)| CellIndex::from(CellCartesian { x, y }));
                assert_eq!(index.offset(dx, dy), expected, "({x}, {y}) by ({dx}, {dy})");
            }
        }
    }

    /// A superchunk's index steps to its neighbours as its cartesian
    /// coordinates would, refusing to step past the world's edges.
    #[test]
    fn superchunk_indices_step_like_cartesian_coordinates() {
        let last = WORLD_SIDE_SUPERCHUNKS - 1;
        let mut random = random();
        // The world's corners and its middle, then superchunks drawn: half anywhere, half on or beside the world's edges.
        let mut superchunks = vec![(0, 0), (1, 0), (MIDDLE, MIDDLE), (MIDDLE - 1, MIDDLE), (last, last), (last, 0), (0, last)];
        let mut coordinate = |by_an_edge: bool| match by_an_edge {
            true => [0, 1, last - 1, last][random.below(4) as usize],
            false => random.below(u64::from(WORLD_SIDE_SUPERCHUNKS)) as u32,
        };
        superchunks.extend((0..100).map(|case| (coordinate(case % 2 == 0), coordinate(case % 4 < 2))));
        let mut steps = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0)];
        steps.extend((0..16).map(|_| (random.below(129) as i32 - 64, random.below(129) as i32 - 64)));
        for (x, y) in superchunks {
            let superchunk = SuperchunkIndex::from_cartesian(x, y);
            for &(dx, dy) in &steps {
                let expected = x
                    .checked_add_signed(dx)
                    .zip(y.checked_add_signed(dy))
                    .filter(|&(x, y)| x < WORLD_SIDE_SUPERCHUNKS && y < WORLD_SIDE_SUPERCHUNKS)
                    .map(|(x, y)| SuperchunkIndex::from_cartesian(x, y));
                assert_eq!(superchunk.offset(dx, dy), expected, "({x}, {y}) by ({dx}, {dy})");
            }
        }
    }
}
