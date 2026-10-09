//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod grass {
    //! Grass over dirt: it spreads onto dirt at its chance, decays at its
    //! chance times its share of grass neighbours, and every cell stays dirt
    //! or grass -- and, for now, only within its limit.
    //!
    //! `cargo test`

    use instructions::handed_on::{BitmapArena, Shape, Write, WriteOp};
    use instructions::handed_on::{grass_on_dirt, DIRT, GRASS};
    use instructions::handed_on::{ChunkStorage, LayerCodec};
    use instructions::handed_on::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};
    use instructions::handed_on::Entities;
    use instructions::Simulation;
    use mc_rules::grass::{tick, DECAY_CHANCE, SPREAD_CHANCE};

    /// The superchunk the tests run on: the world's origin, where grass
    /// grows.
    const SUPERCHUNK: SuperchunkIndex = WORLD_MIDDLE;

    /// Cells in a superchunk.
    const CELLS: u32 = 1 << 20;

    /// An arena with the mock superchunk hot, `grass_cells` cells of grass
    /// scattered on its dirt.
    fn mock(grass_cells: usize) -> BitmapArena {
        let (mut codec, mut arena, mut storage) = (LayerCodec::new(), BitmapArena::new(), ChunkStorage::new(1 << 12));
        storage.insert(SUPERCHUNK, grass_on_dirt(5, grass_cells, &mut codec));
        for chunk in SUPERCHUNK.chunks() {
            arena.make_hot_layers(chunk, &[DIRT, GRASS], &storage, &mut codec);
        }
        arena
    }

    /// Turns the cells of `writes`' shapes to grass.
    fn plant(arena: &mut BitmapArena, writes: impl Iterator<Item = (CellCartesian, Shape)>) {
        for (at, shape) in writes {
            arena.queue(GRASS, Write { at: at.into(), op: WriteOp::Set, shape });
            arena.queue(DIRT, Write { at: at.into(), op: WriteOp::Unset, shape });
        }
        assert_eq!(arena.apply().missed, 0);
    }

    /// The superchunk's first cell, at its top left.
    fn origin() -> CellCartesian {
        SUPERCHUNK.top_left().cartesian()
    }

    /// Grass alone, with no grass around, never decays: a lattice of grass
    /// a cell every other each way -- enough of them for a tick to sample
    /// some -- for a tick.
    #[test]
    fn lone_grass_never_decays() {
        let mut arena = mock(0);
        let cells = (0..512).flat_map(|y| (0..512).map(move |x| CellCartesian { x: origin().x + 2 * x, y: origin().y + 2 * y }));
        plant(&mut arena, cells.map(|cell| (cell, Shape::Cell)));
        assert_eq!(arena.superchunk_count(GRASS, SUPERCHUNK), 512 * 512);
        let done = tick(&mut Simulation::new(1), &mut arena, &mut Entities::new(), 3).rules;
        assert!(done.sampled > 0);
        assert_eq!(done.decays, 0);
    }

    /// Grass with grass all round decays at the whole chance, and has no
    /// dirt to spread onto: a superchunk all grass loses about 0.002% of it
    /// in a tick -- some twenty cells, give or take three times what chance
    /// alone moves that many by; a little less, as cells on its edge see
    /// neighbours past it that are not hot.
    #[test]
    fn surrounded_grass_decays_at_its_chance() {
        let mut arena = mock(0);
        let pieces = (0..8).flat_map(|y| (0..8).map(move |x| CellCartesian { x: origin().x + 128 * x, y: origin().y + 128 * y }));
        plant(&mut arena, pieces.map(|at| (at, Shape::Rect { width: 128, height: 128 })));
        assert_eq!(arena.superchunk_count(GRASS, SUPERCHUNK), CELLS);
        let done = tick(&mut Simulation::new(1), &mut arena, &mut Entities::new(), 4).rules;
        let expected = CELLS as f64 * DECAY_CHANCE;
        assert_eq!(done.spreads, 0);
        assert!((done.decays as f64 - expected).abs() < 3.0 * expected.sqrt(), "{} decays, about {expected:.0} expected", done.decays);
        assert_eq!(arena.superchunk_count(GRASS, SUPERCHUNK), CELLS - done.decays as u32);
    }

    /// Over 1,000 ticks the grass changes by
    /// no more than what spread and decayed, and scattered grass grows --
    /// at most by e, what spreading alone would make of it.
    #[test]
    fn grass_changes_by_what_spread_and_decayed() {
        let mut arena = mock(400);
        let start = arena.superchunk_count(GRASS, SUPERCHUNK);
        let (mut grass, mut simulation) = (start, Simulation::new(1));
        for seed in 0..1000 {
            let done = tick(&mut simulation, &mut arena, &mut Entities::new(), seed).rules;
            let now = arena.superchunk_count(GRASS, SUPERCHUNK);
            assert!(now + done.decays as u32 >= grass && now + done.decays as u32 <= grass + done.spreads as u32, "grown by what spread, less what decayed");
            grass = now;
        }
        let growth = grass as f64 / start as f64;
        assert!(growth > 1.0 && growth < (1000.0 * SPREAD_CHANCE).exp() * 1.1, "grew {growth:.2} times");
    }

    /// Grass grows wherever a superchunk is hot: over the 5x5 superchunks
    /// about the origin, 300 ticks change grass in every one.
    #[test]
    fn grass_grows_in_every_superchunk() {
        let (mut codec, mut arena, mut storage) = (LayerCodec::new(), BitmapArena::new(), ChunkStorage::new(1 << 12));
        let superchunks: Vec<SuperchunkIndex> = (-2..=2).flat_map(|dy| (-2..=2).map(move |dx| WORLD_MIDDLE.offset(dx, dy).expect("in the world"))).collect();
        for (seed, &superchunk) in superchunks.iter().enumerate() {
            storage.insert(superchunk, grass_on_dirt(seed as u64, 300_000, &mut codec));
            arena.make_hot_superchunk(superchunk, &[DIRT, GRASS], &storage, &mut codec);
        }
        // Every chunk's grass, as words.
        let grass = |arena: &BitmapArena| -> Vec<(ChunkIndex, Vec<u64>)> { arena.run(GRASS).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect() };
        let before = grass(&arena);
        let (mut simulation, mut entities) = (Simulation::new(2), Entities::new());
        for seed in 0..300 {
            tick(&mut simulation, &mut arena, &mut entities, seed);
        }
        let after = grass(&arena);
        let mut changed: Vec<CellCartesian> = Vec::new();
        for ((chunk, was), (same, is)) in before.iter().zip(&after) {
            assert_eq!(chunk, same);
            for (word, (&was, &is)) in was.iter().zip(is).enumerate() {
                let flipped = was ^ is;
                changed.extend((0..64).filter(|bit| flipped >> bit & 1 == 1).map(|bit| CellIndex::of(*chunk, word * 64 + bit).cartesian()));
            }
        }
        assert!(changed.len() > 1000, "grass changed on {} cells", changed.len());
        let touched = |superchunk: SuperchunkIndex| changed.iter().any(|&cell| CellIndex::from(cell).superchunk() == superchunk);
        for &superchunk in &superchunks {
            let (x, y) = superchunk.cartesian();
            assert!(touched(superchunk), "{x}, {y}: unchanged");
        }
    }
}
