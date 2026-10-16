//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod sampling {
    //! SCA sampling: every set cell chosen with the probability
    //! asked, only set cells, each once, in Morton order, weighted across
    //! chunks by their counts, and only hot bitmaps.
    //!
    //! `cargo test`

    use bitplane_manager::{BitmapArena, BucketKey, WriteShape, Write, WriteOp};
    use simulation::sample;
    use utilities::chance::Chance;
    use utilities::rng::RandomSource;
    use utilities::seed::counted_seed;
    use chunk_storage::{LayerCodec, LayerType};
    use coordinates::{CellCartesian, CellIndex, ChunkIndex};

    /// The layer type the tests sample.
    const STONE_LAYER: LayerType = LayerType::of_a_bit(4);

    /// Writes setting every cell of the `width` by `height` rectangle from
    /// `(x, y)`, in pieces of at most 128 cells a side.
    fn rect_writes(x: u32, y: u32, width: u32, height: u32) -> Vec<Write> {
        let split_pieces = |piece_start: u32, full_length: u32| (0..full_length.div_ceil(128)).map(move |piece_number| (piece_start + piece_number * 128, (full_length - piece_number * 128).min(128) as u8));
        split_pieces(y, height)
            .flat_map(|(y, height)| split_pieces(x, width).map(move |(x, width)| Write { anchor_cell: CellCartesian { x, y }.into(), write_op: WriteOp::SetCell, write_shape: WriteShape::RectangleOfCells { width, height } }))
            .collect()
    }

    /// An arena with `STONE_LAYER` hot in the chunks at `chunks`, the cells of
    /// `writes` set.
    fn arena_with(chunks: &[ChunkIndex], writes: &[Write]) -> BitmapArena {
        let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
        for &chunk in chunks {
            bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
        }
        for &write in writes {
            bitmap_arena.queue_write(STONE_LAYER, write);
        }
        assert_eq!(bitmap_arena.apply().missed, 0);
        bitmap_arena
    }

    /// Every cell sampled, with `chance`.
    fn sampled_cells(bitmap_arena: &BitmapArena, chance: Chance, seed: u64) -> Vec<CellCartesian> {
        let mut cells = Vec::new();
        let count = sample(bitmap_arena, STONE_LAYER, chance, &mut RandomSource::new(seed), |cell| cells.push(cell.cartesian()));
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
        // The first cell of all, and discs, rectangles and cells drawn anywhere over the two superchunks, their border too.
        let mut seeded_rng = RandomSource::new(counted_seed());
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
        let bitmap_arena = arena_with(&two_superchunks(), &writes);
        let cells = sampled_cells(&bitmap_arena, Chance::ALWAYS_HAPPENS, counted_seed());
        let expected_count: usize = two_superchunks().iter().map(|&chunk| bitmap_arena.bucket(BucketKey { layer_type: STONE_LAYER, chunk }).expect("hot").count() as usize).sum();
        assert_eq!(cells.len(), expected_count);
        assert!(cells.windows(2).all(|cell_pair| CellIndex::from(cell_pair[0]) < CellIndex::from(cell_pair[1])), "in Morton order, each once");
        assert!(cells.iter().all(|&cell| bitmap_arena.layer_holds_cell(STONE_LAYER, cell.into()) == Ok(true)), "only set cells");
        assert!(sampled_cells(&bitmap_arena, Chance::NEVER_HAPPENS, counted_seed()).is_empty());
    }

    /// Each set cell is chosen with the probability asked: over a million
    /// set cells at 1%, the count is within five standard deviations of
    /// 10,486; every cell chosen is set, and they come in Morton order.
    #[test]
    fn each_cell_is_chosen_with_the_probability_asked() {
        let chunks: Vec<ChunkIndex> = (0..4).flat_map(|y| (0..4).map(move |x| chunk_at(102_400 + 256 * x, 102_400 + 256 * y))).collect();
        let bitmap_arena = arena_with(&chunks, &rect_writes(102_400, 102_400, 1024, 1024));
        for seed in (0..3).map(|seed_number| counted_seed().wrapping_add(seed_number)) {
            let cells = sampled_cells(&bitmap_arena, Chance::one_in(100), seed);
            let (expected_mean, standard_deviation) = (1_048_576.0 * 0.01, (1_048_576.0f64 * 0.01 * 0.99).sqrt());
            assert!((cells.len() as f64 - expected_mean).abs() < 5.0 * standard_deviation, "seed {seed}: {} cells", cells.len());
            assert!(cells.windows(2).all(|cell_pair| CellIndex::from(cell_pair[0]) < CellIndex::from(cell_pair[1])));
        }
    }

    /// Samples fall on chunks in proportion to their set cells: a chunk
    /// full and one a sixteenth full get them sixteen to one.
    #[test]
    fn chunks_are_weighted_by_their_counts() {
        let (full_chunk, sparse_chunk) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let writes = [rect_writes(102_400, 102_400, 256, 256), rect_writes(102_656, 102_400, 64, 64)].concat();
        let bitmap_arena = arena_with(&[full_chunk, sparse_chunk], &writes);
        let cells = sampled_cells(&bitmap_arena, Chance::one_in(20), counted_seed());
        let in_sparse = cells.iter().filter(|&&cell| CellIndex::from(cell).chunk() == sparse_chunk).count() as f64;
        let full_to_sparse_ratio = (cells.len() as f64 - in_sparse) / in_sparse;
        assert!((14.0..18.5).contains(&full_to_sparse_ratio), "{full_to_sparse_ratio:.2} to one");
    }

    /// Only hot bitmaps are sampled: an evicted chunk's cells are not.
    #[test]
    fn only_hot_bitmaps_are_sampled() {
        let (kept_chunk, evicted_chunk) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let mut bitmap_arena = arena_with(&[kept_chunk, evicted_chunk], &rect_writes(102_400, 102_400, 512, 4));
        assert_eq!(sampled_cells(&bitmap_arena, Chance::ALWAYS_HAPPENS, counted_seed()).len(), 2048);
        let mut evicted_storage = chunk_storage::ChunkStorage::new(1 << 12);
        bitmap_arena.write_back(evicted_chunk.superchunk(), &mut evicted_storage, &mut LayerCodec::new());
        assert!(bitmap_arena.evict(BucketKey { layer_type: STONE_LAYER, chunk: evicted_chunk }));
        assert!(sampled_cells(&bitmap_arena, Chance::ALWAYS_HAPPENS, counted_seed()).iter().all(|&cell| CellIndex::from(cell).chunk() == kept_chunk));
        assert_eq!(sampled_cells(&bitmap_arena, Chance::ALWAYS_HAPPENS, counted_seed()).len(), 1024);
    }
}

mod tick {
    //! The tick: rules run superchunk by superchunk, writes queued for the
    //! superchunks they land in and applied in a second phase -- the same on
    //! any number of threads, across borders, reading the world as the tick
    //! found it, and never past the superchunks next door.
    //!
    //! `cargo test`

    use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
    use entity_manager::WorldEntities;
    use simulation::{Simulation, Turn};
    use utilities::chance::Chance;
    use chunk_storage::{LayerCodec, LayerType};
    use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};

    /// The layer type the tests run on.
    const STONE_LAYER: LayerType = LayerType::of_a_bit(6);

    /// An arena with `STONE_LAYER` hot over the `side_superchunks` by `side_superchunks` superchunks from
    /// `(10, 10)`, the cells `cells` set.
    fn stone_arena(side_superchunks: u32, cells: impl Iterator<Item = CellCartesian>) -> BitmapArena {
        let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
        for y in 10..10 + side_superchunks {
            for x in 10..10 + side_superchunks {
                for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                    bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
                }
            }
        }
        cells.for_each(|cell| bitmap_arena.queue_write(STONE_LAYER, Write::cell(cell.into(), WriteOp::SetCell)));
        assert_eq!(bitmap_arena.apply().missed, 0);
        bitmap_arena
    }

    /// The top left cell of the superchunk `(x, y)`, cartesian.
    fn top_left_cartesian(x: u32, y: u32) -> CellCartesian {
        SuperchunkIndex::from_cartesian(x, y).top_left().cartesian()
    }

    /// Stone creeping: each cell sampled at 5% sets a random neighbour it
    /// sees clear -- within the 3x3 around it -- or clears itself: how
    /// many it sampled.
    fn stone_creep(turn: &mut Turn) -> usize {
        turn.each_sampled(STONE_LAYER, Chance::one_in(20), &mut (), |turn, cell, _| {
            let (dx, dy) = (turn.random_stream().draw_below(3) as i32 - 1, turn.random_stream().draw_below(3) as i32 - 1);
            if (dx, dy) == (0, 0) {
                actions::cells::seen_becoming(turn, STONE_LAYER, cell, 1, 0, None);
            } else if let Some(clear_neighbour) = cell.at_cartesian_offset(dx, dy).filter(|&candidate_neighbour| turn.layer_holds(STONE_LAYER, candidate_neighbour) == Ok(false)) {
                actions::cells::seen_becoming(turn, STONE_LAYER, clear_neighbour, 0, 1, None);
            }
        })
    }

    /// Every hot bitmap's cells, in the arena's order.
    fn every_cell(bitmap_arena: &BitmapArena) -> Vec<(BucketKey, Vec<u64>)> {
        bitmap_arena.hot_bucket_keys().map(|bucket_key| (bucket_key, bitmap_arena.bucket(bucket_key).expect("hot").cells().to_vec())).collect()
    }

    /// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
    /// their borders.
    fn scattered_cells() -> impl Iterator<Item = CellCartesian> {
        let (start_cartesian, mut seeded_rng) = (top_left_cartesian(10, 10), utilities::rng::RandomSource::new(utilities::seed::counted_seed()));
        // One in four on a superchunk's border, each way.
        let mut draw_coordinate = move || if seeded_rng.draw_below(4) == 0 { (seeded_rng.draw_below(4) * 1024).saturating_sub(seeded_rng.draw_below(2)).min(3071) as u32 } else { seeded_rng.draw_below(3072) as u32 };
        (0..3000).map(move |_| CellCartesian { x: start_cartesian.x + draw_coordinate(), y: start_cartesian.y + draw_coordinate() })
    }

    /// A tick comes out the same on one thread and on four: each
    /// superchunk's random numbers are its own, and the second phase applies
    /// in a fixed order.
    #[test]
    fn any_number_of_threads_ticks_the_same() {
        let (mut one_thread_arena, mut four_thread_arena) = (stone_arena(3, scattered_cells()), stone_arena(3, scattered_cells()));
        let seed = utilities::seed::counted_seed();
        for tick in 0..20 {
            let (one_thread_report, four_thread_report) = (Simulation::new(1).tick(&mut one_thread_arena, &mut WorldEntities::new(), seed, stone_creep), Simulation::new(4).tick(&mut four_thread_arena, &mut WorldEntities::new(), seed, stone_creep));
            assert_eq!((one_thread_report.rules, one_thread_report.writes_applied), (four_thread_report.rules, four_thread_report.writes_applied), "tick {tick}");
        }
        assert_eq!(every_cell(&one_thread_arena), every_cell(&four_thread_arena));
    }

    /// A write lands in the neighbour it falls in; reads in the first phase
    /// see the world as the tick found it, writes queued or not.
    #[test]
    fn writes_cross_borders_and_reads_see_the_tick_start() {
        // Any cell down the superchunk's east edge.
        let east_border_cell = CellCartesian { x: top_left_cartesian(11, 10).x - 1, y: top_left_cartesian(10, 10).y + utilities::rng::RandomSource::new(utilities::seed::counted_seed()).draw_below(1024) as u32 };
        let mut bitmap_arena = stone_arena(2, [east_border_cell].into_iter());
        let across_cell: CellIndex = CellCartesian { x: east_border_cell.x + 1, y: east_border_cell.y }.into();
        let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
            turn.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |turn, cell, _| {
                let east_neighbour = cell.at_cartesian_offset(1, 0).expect("in the world");
                actions::cells::seen_becoming(turn, STONE_LAYER, east_neighbour, 0, 1, None);
                assert_eq!(turn.layer_holds(STONE_LAYER, east_neighbour), Ok(false), "still as the tick found it");
            })
        });
        assert_eq!((report.rules, report.writes_applied.cells_changed), (1, 1));
        assert_eq!(bitmap_arena.layer_holds_cell(STONE_LAYER, across_cell), Ok(true), "set in the neighbour");
        assert_eq!(bitmap_arena.superchunk_count(STONE_LAYER, SuperchunkIndex::from_cartesian(11, 10)), 1);
    }

    /// The cells of a square straddling the corner four superchunks meet
    /// at, written one by one, land in all four, each applying its own.
    #[test]
    fn writes_about_a_corner_land_in_the_four_superchunks() {
        let meeting_corner = top_left_cartesian(11, 11);
        let mut bitmap_arena = stone_arena(2, [CellCartesian { x: meeting_corner.x - 1, y: meeting_corner.y - 1 }].into_iter());
        let report = Simulation::new(2).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
            turn.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |turn, cell, _| {
                for (x, y) in (-1..3).flat_map(|y| (-1..3).map(move |x| (x, y))) {
                    let square_cell = cell.at_cartesian_offset(x, y).expect("in the world");
                    if turn.layer_holds(STONE_LAYER, square_cell) == Ok(false) {
                        actions::cells::seen_becoming(turn, STONE_LAYER, square_cell, 0, 1, None);
                    }
                }
            });
            0
        });
        assert_eq!(report.writes_applied.cells_changed, 15, "the 4x4 from two up and left of the meeting point, one cell set already");
        for (x, y, cells) in [(10, 10, 4), (11, 10, 4), (10, 11, 4), (11, 11, 4)] {
            assert_eq!(bitmap_arena.superchunk_count(STONE_LAYER, SuperchunkIndex::from_cartesian(x, y)), cells, "superchunk ({x}, {y})");
        }
    }

    /// Writes landing in a superchunk with no bitmap in use are missed, and
    /// counted.
    #[test]
    fn writes_to_cold_neighbours_are_missed() {
        let east_border_cell = CellCartesian { x: top_left_cartesian(11, 10).x - 1, y: top_left_cartesian(10, 10).y + utilities::rng::RandomSource::new(utilities::seed::counted_seed()).draw_below(1024) as u32 };
        let mut bitmap_arena = stone_arena(1, [east_border_cell].into_iter());
        let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
            turn.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |turn, cell, _| {
                actions::cells::seen_becoming(turn, STONE_LAYER, cell.at_cartesian_offset(1, 0).expect("in the world"), 0, 1, None);
            });
            0
        });
        assert_eq!((report.writes_applied.cells_changed, report.writes_applied.missed), (0, 1));
    }

    /// A write two superchunks away is past the speed of light.
    #[test]
    #[should_panic(expected = "past the speed of light")]
    fn writes_past_the_speed_of_light_panic() {
        let mut bitmap_arena = stone_arena(1, [top_left_cartesian(10, 10)].into_iter());
        Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
            turn.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |turn, cell, _| {
                actions::cells::seen_becoming(turn, STONE_LAYER, cell.at_cartesian_offset(2 * SUPERCHUNK_SIDE_CELLS as i32, 0).expect("in the world"), 0, 1, None);
            });
            0
        });
    }
}
