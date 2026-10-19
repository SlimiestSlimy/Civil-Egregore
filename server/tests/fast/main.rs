//! The server's fast tier: the simulation's cells sampled from bitmaps grown from a seed -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test -p server --lib fast_tests`

// What the tiers share is declared at the crate's root, beside them.

mod around;

mod area;

mod mask;

mod tick;

mod sampling {
    //! SCA sampling: every set cell chosen with the probability
    //! asked, only set cells, each once, in Morton order, weighted across
    //! chunks by their counts, and only hot bitmaps.
    //!
    //! `cargo test`

    use crate::bitplane_manager::{BitmapArena, BucketKey, WriteShape, Write, WriteOp};
    use crate::test_types::TestTypes;
    use crate::simulation::sample;
    use utilities::chance::Chance;
    use utilities::rng::RandomSource;
    use crate::chunk_storage::LayerCodec;
    use utilities::coordinates::{CellCartesian, CellIndex, ChunkIndex};


    /// Writes setting every cell of the `width` by `height` rectangle from
    /// `(x, y)`, in pieces of at most 128 cells a side.
    fn rect_writes(x: u32, y: u32, width: u32, height: u32) -> Vec<Write> {
        let split_pieces = |piece_start: u32, full_length: u32| (0..full_length.div_ceil(128)).map(move |piece_number| (piece_start + piece_number * 128, (full_length - piece_number * 128).min(128) as u8));
        split_pieces(y, height)
            .flat_map(|(y, height)| split_pieces(x, width).map(move |(x, width)| Write { anchor_cell: CellCartesian { x, y }.into(), write_op: WriteOp::SetCell, write_shape: WriteShape::RectangleOfCells { width, height } }))
            .collect()
    }

    /// An arena with `types.stone` hot in the chunks at `chunks`, the cells of
    /// `writes` set.
    fn arena_with(types: TestTypes, chunks: &[ChunkIndex], writes: &[Write]) -> BitmapArena {
        let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
        for &chunk in chunks {
            bitmap_arena.make_hot(BucketKey { layer_type: types.stone, chunk }, None, &mut layer_codec);
        }
        for &write in writes {
            bitmap_arena.queue_write(types.stone, write);
        }
        assert_eq!(bitmap_arena.apply().missed, 0);
        bitmap_arena
    }

    /// Every cell sampled, with `chance`.
    fn sampled_cells(types: TestTypes, bitmap_arena: &BitmapArena, chance: Chance, seed: u64) -> Vec<CellCartesian> {
        let mut cells = Vec::new();
        let count = sample(bitmap_arena, types.stone, chance, &mut RandomSource::new(seed), |cell| cells.push(cell.cartesian()));
        assert_eq!(count, cells.len());
        cells
    }

    /// The chunk whose top left cell is `x` and `y` cells from the world's.
    fn chunk_at(x: u32, y: u32) -> ChunkIndex {
        CellIndex::from(CellCartesian { x, y }).chunk()
    }

    /// The chunks of the two superchunks the tests use, side by side.
    fn two_superchunks() -> Vec<ChunkIndex> {
        (0..4).flat_map(|y| (0..8).map(move |x| chunk_at(102_400 + 256 * x, 102_400 + 256 * y))).collect()
    }

    /// At probability 1 every set cell comes once, in Morton order across
    /// superchunks; at 0, none.
    #[test]
    fn certain_sampling_finds_every_set_cell_in_morton_order() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let types = TestTypes::seeded(seed);
        // The first cell of all, and discs, rectangles and cells drawn anywhere over the two superchunks, their border too.
        let mut seeded_rng = RandomSource::new(seed);
        let mut writes = vec![Write::cell(CellCartesian { x: 102_400, y: 102_400 }.into(), WriteOp::SetCell)];
        for _ in 0..40 {
            // Far enough in that the widest shape stays on the two superchunks.
            let shape_cell = CellCartesian { x: 102_400 + 60 + seeded_rng.draw_below(2048 - 60 - 256) as u32, y: 102_400 + 60 + seeded_rng.draw_below(1024 - 60 - 256) as u32 };
            let drawn_shape = match seeded_rng.draw_below(3) {
                0 => WriteShape::DiscOfCells { radius_in_cells: seeded_rng.draw_below(60) as u8 },
                1 => WriteShape::RectangleOfCells { width: seeded_rng.draw_between(1, 255) as u8, height: seeded_rng.draw_between(1, 255) as u8 },
                _ => WriteShape::Cell,
            };
            writes.push(Write { anchor_cell: shape_cell.into(), write_op: WriteOp::SetCell, write_shape: drawn_shape });
        }
        let bitmap_arena = arena_with(types, &two_superchunks(), &writes);
        let cells = sampled_cells(types, &bitmap_arena, Chance::ALWAYS_HAPPENS, seed);
        let expected_count: usize = two_superchunks().iter().map(|&chunk| bitmap_arena.bucket(BucketKey { layer_type: types.stone, chunk }).expect("hot").count() as usize).sum();
        assert_eq!(cells.len(), expected_count);
        assert!(cells.windows(2).all(|cell_pair| CellIndex::from(cell_pair[0]) < CellIndex::from(cell_pair[1])), "in Morton order, each once");
        assert!(cells.iter().all(|&cell| bitmap_arena.layer_holds_cell(types.stone, cell.into()) == Ok(true)), "only set cells");
        assert!(sampled_cells(types, &bitmap_arena, Chance::NEVER_HAPPENS, seed).is_empty());
    }

    /// Each set cell is chosen with the probability asked: over a million
    /// set cells at 1%, the count is within five standard deviations of
    /// 10,486; every cell chosen is set, and they come in Morton order.
    #[test]
    fn each_cell_is_chosen_with_the_probability_asked() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let types = TestTypes::seeded(seed);
        let chunks: Vec<ChunkIndex> = (0..4).flat_map(|y| (0..4).map(move |x| chunk_at(102_400 + 256 * x, 102_400 + 256 * y))).collect();
        let bitmap_arena = arena_with(types, &chunks, &rect_writes(102_400, 102_400, 1024, 1024));
        for seed in (0..3).map(|seed_number| seed.wrapping_add(seed_number)) {
            let cells = sampled_cells(types, &bitmap_arena, Chance::one_in(100), seed);
            let (expected_mean, standard_deviation) = (1_048_576.0 * 0.01, (1_048_576.0f64 * 0.01 * 0.99).sqrt());
            assert!((cells.len() as f64 - expected_mean).abs() < 5.0 * standard_deviation, "seed {seed}: {} cells", cells.len());
            assert!(cells.windows(2).all(|cell_pair| CellIndex::from(cell_pair[0]) < CellIndex::from(cell_pair[1])));
        }
    }

    /// Samples fall on chunks in proportion to their set cells: a chunk
    /// full and one a sixteenth full get them sixteen to one -- each
    /// within five standard deviations of its share.
    #[test]
    fn chunks_are_weighted_by_their_counts() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let types = TestTypes::seeded(seed);
        let (full_chunk, sparse_chunk) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let writes = [rect_writes(102_400, 102_400, 256, 256), rect_writes(102_656, 102_400, 64, 64)].concat();
        let bitmap_arena = arena_with(types, &[full_chunk, sparse_chunk], &writes);
        let cells = sampled_cells(types, &bitmap_arena, Chance::one_in(20), seed);
        let in_sparse = cells.iter().filter(|&&cell| CellIndex::from(cell).chunk() == sparse_chunk).count() as f64;
        let in_full = cells.len() as f64 - in_sparse;
        // Each chunk's samples: its set cells at one in 20, binomial.
        let within_five_deviations = |sampled: f64, set_cells: f64| (sampled - set_cells / 20.0).abs() < 5.0 * (set_cells / 20.0 * 0.95).sqrt();
        assert!(within_five_deviations(in_full, 65_536.0) && within_five_deviations(in_sparse, 4_096.0), "{in_full} in the full chunk, {in_sparse} in the sparse: {:.2} to one", in_full / in_sparse);
    }

    /// Only hot bitmaps are sampled: an evicted chunk's cells are not.
    #[test]
    fn only_hot_bitmaps_are_sampled() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let types = TestTypes::seeded(seed);
        let (kept_chunk, evicted_chunk) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let mut bitmap_arena = arena_with(types, &[kept_chunk, evicted_chunk], &rect_writes(102_400, 102_400, 512, 4));
        assert_eq!(sampled_cells(types, &bitmap_arena, Chance::ALWAYS_HAPPENS, seed).len(), 2048);
        let mut evicted_storage = crate::chunk_storage::ChunkStorage::new(1 << 12);
        bitmap_arena.write_back(evicted_chunk.superchunk(), &mut evicted_storage, &mut LayerCodec::new());
        assert!(bitmap_arena.evict(BucketKey { layer_type: types.stone, chunk: evicted_chunk }));
        assert!(sampled_cells(types, &bitmap_arena, Chance::ALWAYS_HAPPENS, seed).iter().all(|&cell| CellIndex::from(cell).chunk() == kept_chunk));
        assert_eq!(sampled_cells(types, &bitmap_arena, Chance::ALWAYS_HAPPENS, seed).len(), 1024);
    }
}
