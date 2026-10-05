//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
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
    use coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, CHUNK_SIDE, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};

    /// The cartesian coordinate, each way, of the superchunk in the middle
    /// of the world.
    const MIDDLE: u32 = WORLD_SIDE_SUPERCHUNKS / 2;

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
        assert_eq!(key(1234, last).cartesian(), (1234, last));
    }

    /// A cell's index nests its chunk's and superchunk's: from the lowest
    /// bit, 16 for its place in its chunk, 4 for its chunk's place in its
    /// superchunk, 44 for its superchunk -- and each comes back from its
    /// parts, and the cell from its cartesian coordinates.
    #[test]
    fn a_cells_index_nests_its_chunks_and_superchunks() {
        let middle = MIDDLE * SUPERCHUNK_SIDE_CELLS;
        let chunk_side = CHUNK_SIDE as u32;
        let edges = [0, 1, chunk_side - 1, chunk_side, SUPERCHUNK_SIDE_CELLS - 1, SUPERCHUNK_SIDE_CELLS, 5 * SUPERCHUNK_SIDE_CELLS + 1234];
        let coordinates: Vec<u32> = edges.iter().flat_map(|&edge| [edge, middle + edge, middle - edge - 1, u32::MAX - edge]).collect();
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
        let cells = [(0, 0), (1, 0), (255, 256), (edge - 1, edge), (edge + 1023, edge + 1023), (u32::MAX, u32::MAX), (u32::MAX, 0), (0, u32::MAX), (12345, 678910), (1, 1), (254, 254), (300, 511), (257, 300), (100, 255)];
        for (x, y) in cells {
            let index = CellIndex::from(CellCartesian { x, y });
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0), (-300, 77), (1024, -1025), (8, 0), (0, -8), (-8, 8), (256, 2), (-4, 64)] {
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
        for (x, y) in [(0, 0), (1, 0), (MIDDLE, MIDDLE), (MIDDLE - 1, MIDDLE), (last, last), (last, 0), (0, last), (1234, 98765)] {
            let superchunk = SuperchunkIndex::from_cartesian(x, y);
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1), (0, 0), (-3, 5), (64, -2)] {
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
