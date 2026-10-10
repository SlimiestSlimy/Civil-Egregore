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

    use bitplane_manager::{BitmapArena, BucketKey, Shape, Write, WriteOp};
    use simulation::sample;
    use utilities::chance::Chance;
    use utilities::rng::Rng;
    use utilities::seed::counted;
    use chunk_storage::{LayerCodec, LayerType};
    use coordinates::{CellCartesian, CellIndex, ChunkIndex};

    /// The layer type the tests sample.
    const STONE: LayerType = LayerType(4);

    /// Writes setting every cell of the `width` by `height` rectangle from
    /// `(x, y)`, in pieces of at most 128 cells a side.
    fn rect(x: u32, y: u32, width: u32, height: u32) -> Vec<Write> {
        let piece = |start: u32, length: u32| (0..length.div_ceil(128)).map(move |at| (start + at * 128, (length - at * 128).min(128) as u8));
        piece(y, height)
            .flat_map(|(y, height)| piece(x, width).map(move |(x, width)| Write { at: CellCartesian { x, y }.into(), op: WriteOp::Set, shape: Shape::Rect { width, height } }))
            .collect()
    }

    /// An arena with `STONE` hot in the chunks at `chunks`, the cells of
    /// `writes` set.
    fn arena_with(chunks: &[ChunkIndex], writes: &[Write]) -> BitmapArena {
        let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
        for &chunk in chunks {
            arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
        }
        for &write in writes {
            arena.queue(STONE, write);
        }
        assert_eq!(arena.apply().missed, 0);
        arena
    }

    /// Every cell sampled, with `chance`.
    fn sampled(arena: &BitmapArena, chance: Chance, seed: u64) -> Vec<CellCartesian> {
        let mut cells = Vec::new();
        let count = sample(arena, STONE, chance, &mut Rng::new(seed), |cell| cells.push(cell.cartesian()));
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
        let mut random = Rng::new(counted());
        let mut writes = vec![Write::cell(CellCartesian { x: 102_400, y: 102_400 }.into(), WriteOp::Set)];
        for _ in 0..40 {
            // Far enough in that the widest shape stays on the two superchunks.
            let at = CellCartesian { x: 102_400 + 60 + random.below(2048 - 60 - 256) as u32, y: 102_400 + 60 + random.below(1024 - 60 - 256) as u32 };
            let shape = match random.below(3) {
                0 => Shape::Disc { radius: random.below(60) as u8 },
                1 => Shape::Rect { width: random.between(1, 255) as u8, height: random.between(1, 255) as u8 },
                _ => Shape::Cell,
            };
            writes.push(Write { at: at.into(), op: WriteOp::Set, shape });
        }
        let arena = arena_with(&two_superchunks(), &writes);
        let cells = sampled(&arena, Chance::ALWAYS, counted());
        let expected: usize = two_superchunks().iter().map(|&chunk| arena.bucket(BucketKey { layer_type: STONE, chunk }).expect("hot").count() as usize).sum();
        assert_eq!(cells.len(), expected);
        assert!(cells.windows(2).all(|pair| CellIndex::from(pair[0]) < CellIndex::from(pair[1])), "in Morton order, each once");
        assert!(cells.iter().all(|&cell| arena.holds(STONE, cell.into()) == Ok(true)), "only set cells");
        assert!(sampled(&arena, Chance::NEVER, counted()).is_empty());
    }

    /// Each set cell is chosen with the probability asked: over a million
    /// set cells at 1%, the count is within five standard deviations of
    /// 10,486; every cell chosen is set, and they come in Morton order.
    #[test]
    fn each_cell_is_chosen_with_the_probability_asked() {
        let chunks: Vec<ChunkIndex> = (0..4).flat_map(|y| (0..4).map(move |x| chunk_at(102_400 + 256 * x, 102_400 + 256 * y))).collect();
        let arena = arena_with(&chunks, &rect(102_400, 102_400, 1024, 1024));
        for seed in (0..3).map(|nth| counted().wrapping_add(nth)) {
            let cells = sampled(&arena, Chance::one_in(100), seed);
            let (mean, deviation) = (1_048_576.0 * 0.01, (1_048_576.0f64 * 0.01 * 0.99).sqrt());
            assert!((cells.len() as f64 - mean).abs() < 5.0 * deviation, "seed {seed}: {} cells", cells.len());
            assert!(cells.windows(2).all(|pair| CellIndex::from(pair[0]) < CellIndex::from(pair[1])));
        }
    }

    /// Samples fall on chunks in proportion to their set cells: a chunk
    /// full and one a sixteenth full get them sixteen to one.
    #[test]
    fn chunks_are_weighted_by_their_counts() {
        let (full, sparse) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let writes = [rect(102_400, 102_400, 256, 256), rect(102_656, 102_400, 64, 64)].concat();
        let arena = arena_with(&[full, sparse], &writes);
        let cells = sampled(&arena, Chance::one_in(20), counted());
        let in_sparse = cells.iter().filter(|&&cell| CellIndex::from(cell).chunk() == sparse).count() as f64;
        let ratio = (cells.len() as f64 - in_sparse) / in_sparse;
        assert!((14.0..18.5).contains(&ratio), "{ratio:.2} to one");
    }

    /// Only hot bitmaps are sampled: an evicted chunk's cells are not.
    #[test]
    fn only_hot_bitmaps_are_sampled() {
        let (kept, evicted) = (chunk_at(102_400, 102_400), chunk_at(102_656, 102_400));
        let mut arena = arena_with(&[kept, evicted], &rect(102_400, 102_400, 512, 4));
        assert_eq!(sampled(&arena, Chance::ALWAYS, counted()).len(), 2048);
        let mut storage = chunk_storage::ChunkStorage::new(1 << 12);
        arena.write_back(evicted.superchunk(), &mut storage, &mut LayerCodec::new());
        assert!(arena.evict(BucketKey { layer_type: STONE, chunk: evicted }));
        assert!(sampled(&arena, Chance::ALWAYS, counted()).iter().all(|&cell| CellIndex::from(cell).chunk() == kept));
        assert_eq!(sampled(&arena, Chance::ALWAYS, counted()).len(), 1024);
    }
}

mod tick {
    //! The tick: rules run superchunk by superchunk, writes queued for the
    //! superchunks they land in and applied in a second phase -- the same on
    //! any number of threads, across borders, reading the world as the tick
    //! found it, and never past the superchunks next door.
    //!
    //! `cargo test`

    use bitplane_manager::{BitmapArena, BucketKey, Shape, Write, WriteOp};
    use entity_manager::Entities;
    use simulation::{Simulation, Turn};
    use utilities::chance::Chance;
    use chunk_storage::{LayerCodec, LayerType};
    use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};

    /// The layer type the tests run on.
    const STONE: LayerType = LayerType(6);

    /// An arena with `STONE` hot over the `side` by `side` superchunks from
    /// `(10, 10)`, the cells `cells` set.
    fn arena(side: u32, cells: impl Iterator<Item = CellCartesian>) -> BitmapArena {
        let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
        for y in 10..10 + side {
            for x in 10..10 + side {
                for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                    arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
                }
            }
        }
        cells.for_each(|cell| arena.queue(STONE, Write::cell(cell.into(), WriteOp::Set)));
        assert_eq!(arena.apply().missed, 0);
        arena
    }

    /// The top left cell of the superchunk `(x, y)`, cartesian.
    fn corner(x: u32, y: u32) -> CellCartesian {
        SuperchunkIndex::from_cartesian(x, y).top_left().cartesian()
    }

    /// Stone creeping: each cell sampled at 5% sets a random neighbour --
    /// within the 3x3 around it -- or clears itself: how many it sampled.
    fn creep(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> usize {
        let sampled = turn.sample(STONE, Chance::one_in(20), samples);
        for &cell in samples.iter() {
            let (dx, dy) = (turn.random().below(3) as i32 - 1, turn.random().below(3) as i32 - 1);
            if (dx, dy) == (0, 0) {
                turn.queue(STONE, Write::cell(cell, WriteOp::Unset));
            } else if let Some(neighbour) = cell.offset(dx, dy) {
                turn.queue(STONE, Write::cell(neighbour, WriteOp::Set));
            }
        }
        sampled
    }

    /// Every hot bitmap's cells, in the arena's order.
    fn every_cell(arena: &BitmapArena) -> Vec<(BucketKey, Vec<u64>)> {
        arena.keys().map(|key| (key, arena.bucket(key).expect("hot").cells().to_vec())).collect()
    }

    /// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
    /// their borders.
    fn scattered() -> impl Iterator<Item = CellCartesian> {
        let (start, mut random) = (corner(10, 10), utilities::rng::Rng::new(utilities::seed::counted()));
        // One in four on a superchunk's border, each way.
        let mut coordinate = move || if random.below(4) == 0 { (random.below(4) * 1024).saturating_sub(random.below(2)).min(3071) as u32 } else { random.below(3072) as u32 };
        (0..3000).map(move |_| CellCartesian { x: start.x + coordinate(), y: start.y + coordinate() })
    }

    /// A tick comes out the same on one thread and on four: each
    /// superchunk's random numbers are its own, and the second phase applies
    /// in a fixed order.
    #[test]
    fn any_number_of_threads_ticks_the_same() {
        let (mut one, mut four) = (arena(3, scattered()), arena(3, scattered()));
        let seed = utilities::seed::counted();
        for tick in 0..20 {
            let (a, b) = (Simulation::new(1).tick(&mut one, &mut Entities::new(), seed, creep), Simulation::new(4).tick(&mut four, &mut Entities::new(), seed, creep));
            assert_eq!((a.rules, a.writes_applied), (b.rules, b.writes_applied), "tick {tick}");
        }
        assert_eq!(every_cell(&one), every_cell(&four));
    }

    /// A write lands in the neighbour it falls in; reads in the first phase
    /// see the world as the tick found it, writes queued or not.
    #[test]
    fn writes_cross_borders_and_reads_see_the_tick_start() {
        // Any cell down the superchunk's east edge.
        let edge = CellCartesian { x: corner(11, 10).x - 1, y: corner(10, 10).y + utilities::rng::Rng::new(utilities::seed::counted()).below(1024) as u32 };
        let mut arena = arena(2, [edge].into_iter());
        let across: CellIndex = CellCartesian { x: edge.x + 1, y: edge.y }.into();
        let report = Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, samples| {
            turn.sample(STONE, Chance::ALWAYS, samples);
            for &cell in samples.iter() {
                let right = cell.offset(1, 0).expect("in the world");
                turn.queue(STONE, Write::cell(right, WriteOp::Set));
                assert_eq!(turn.holds(STONE, right), Ok(false), "still as the tick found it");
            }
            samples.len()
        });
        assert_eq!((report.rules, report.writes_applied.changed), (1, 1));
        assert_eq!(arena.holds(STONE, across), Ok(true), "set in the neighbour");
        assert_eq!(arena.superchunk_count(STONE, SuperchunkIndex::from_cartesian(11, 10)), 1);
    }

    /// A rectangle straddling the corner four superchunks meet at lands in
    /// all four, each applying its own part.
    #[test]
    fn shapes_split_over_the_superchunks_they_cover() {
        let meet = corner(11, 11);
        let mut arena = arena(2, [CellCartesian { x: meet.x - 1, y: meet.y - 1 }].into_iter());
        let report = Simulation::new(2).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, samples| {
            turn.sample(STONE, Chance::ALWAYS, samples);
            for &cell in samples.iter() {
                turn.queue(STONE, Write { at: cell.offset(-1, -1).expect("in the world"), op: WriteOp::Set, shape: Shape::Rect { width: 4, height: 4 } });
            }
            0
        });
        assert_eq!(report.writes_applied.changed, 15, "the 4x4 from two up and left of the meeting point, one cell set already");
        for (x, y, cells) in [(10, 10, 4), (11, 10, 4), (10, 11, 4), (11, 11, 4)] {
            assert_eq!(arena.superchunk_count(STONE, SuperchunkIndex::from_cartesian(x, y)), cells, "superchunk ({x}, {y})");
        }
    }

    /// Writes landing in a superchunk with no bitmap in use are missed, and
    /// counted.
    #[test]
    fn writes_to_cold_neighbours_are_missed() {
        let edge = CellCartesian { x: corner(11, 10).x - 1, y: corner(10, 10).y + utilities::rng::Rng::new(utilities::seed::counted()).below(1024) as u32 };
        let mut arena = arena(1, [edge].into_iter());
        let report = Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, samples| {
            turn.sample(STONE, Chance::ALWAYS, samples);
            for &cell in samples.iter() {
                turn.queue(STONE, Write::cell(cell.offset(1, 0).expect("in the world"), WriteOp::Set));
            }
            0
        });
        assert_eq!((report.writes_applied.changed, report.writes_applied.missed), (0, 1));
    }

    /// A write two superchunks away is past the speed of light.
    #[test]
    #[should_panic(expected = "past the speed of light")]
    fn writes_past_the_speed_of_light_panic() {
        let mut arena = arena(1, [corner(10, 10)].into_iter());
        Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, samples| {
            turn.sample(STONE, Chance::ALWAYS, samples);
            for &cell in samples.iter() {
                turn.queue(STONE, Write::cell(cell.offset(2 * SUPERCHUNK_SIDE_CELLS as i32, 0).expect("in the world"), WriteOp::Set));
            }
            0
        });
    }
}
