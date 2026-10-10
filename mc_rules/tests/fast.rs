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

    use instructions::layers::GRASS;
    use instructions::mock_world::MockWorld;
    use instructions::{CellCartesian, CellIndex, SuperchunkIndex};
    use mc_rules::grass::{rule, DECAY_CHANCE, SPREAD_CHANCE};

    /// Cells in a superchunk.
    const CELLS: u64 = 1 << 20;

    /// A mock world of one superchunk, `grass_cells` cells of grass
    /// scattered on its dirt.
    fn mock(grass_cells: usize) -> MockWorld {
        MockWorld::grass_on_dirt(1, grass_cells)
    }

    /// The first cell of `world`, at its top left.
    fn origin(world: &MockWorld) -> CellCartesian {
        world.superchunks()[0].top_left().cartesian()
    }

    /// Grass alone, with no grass around, never decays: a lattice of grass
    /// a cell every other each way -- enough of them for a tick to sample
    /// some -- for a tick.
    #[test]
    fn lone_grass_never_decays() {
        let mut world = mock(0);
        let origin = origin(&world);
        for (x, y) in (0..512).flat_map(|y| (0..512).map(move |x| (x, y))) {
            world.plant_grass(CellCartesian { x: origin.x + 2 * x, y: origin.y + 2 * y }, 1, 1);
        }
        assert_eq!(world.count(GRASS), 512 * 512);
        let done = world.tick(3, rule).rules;
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
        let mut world = mock(0);
        let origin = origin(&world);
        for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
            world.plant_grass(CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
        }
        assert_eq!(world.count(GRASS), CELLS);
        let done = world.tick(4, rule).rules;
        let expected = CELLS as f64 * DECAY_CHANCE;
        assert_eq!(done.spreads, 0);
        assert!((done.decays as f64 - expected).abs() < 3.0 * expected.sqrt(), "{} decays, about {expected:.0} expected", done.decays);
        assert_eq!(world.count(GRASS), CELLS - done.decays as u64);
    }

    /// Over 1,000 ticks the grass changes by
    /// no more than what spread and decayed, and scattered grass grows --
    /// at most by e, what spreading alone would make of it.
    #[test]
    fn grass_changes_by_what_spread_and_decayed() {
        let mut world = mock(400);
        let start = world.count(GRASS);
        let mut grass = start;
        for seed in 0..1000 {
            let done = world.tick(seed, rule).rules;
            let now = world.count(GRASS);
            assert!(now + done.decays as u64 >= grass && now + done.decays as u64 <= grass + done.spreads as u64, "grown by what spread, less what decayed");
            grass = now;
        }
        let growth = grass as f64 / start as f64;
        assert!(growth > 1.0 && growth < (1000.0 * SPREAD_CHANCE).exp() * 1.1, "grew {growth:.2} times");
    }

    /// Grass grows wherever a superchunk is hot: over 5x5 superchunks
    /// from the origin, 300 ticks change grass in every one.
    #[test]
    fn grass_grows_in_every_superchunk() {
        let mut world = MockWorld::grass_on_dirt(25, 300_000).on_threads(2);
        let superchunks: Vec<SuperchunkIndex> = world.superchunks().to_vec();
        // Every chunk's grass, as words.
        let before = world.words(GRASS);
        for seed in 0..300 {
            world.tick(seed, rule);
        }
        let after = world.words(GRASS);
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
