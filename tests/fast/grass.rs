//! Grass over dirt, on a plain the server makes, by itself: it spreads
//! onto dirt at its chance, and turns back to dirt only where grass
//! crowds it.

use protocol::server_parameters::{ParameterValue, SEED};
use crate::worlds::{cells_of_grass, first_superchunk, plain_start, plant_grass, tick_grass};
use utilities::coordinates::CellCartesian;
use basegame::sca_rules::grass::{DECAYS_COUNT, DECAY_CHANCE, SPREADS_COUNT, SPREAD_CHANCE};
use server::World;
use utilities::rng::RandomSource;

/// Cells in a superchunk.
const CELLS: u64 = 1 << 20;

/// The first cell of `world`, at its top left.
fn origin(world: &World) -> CellCartesian {
    first_superchunk(world).top_left().cartesian()
}

/// A superchunk all grass, with no dirt to spread onto: over some ticks
/// it loses the share of it its chance of decay says -- every
/// neighbour drawn is grass, but at the world's edge -- give or take
/// four times what chance alone moves that many by.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_crowded_by_grass_decays_at_its_chance() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let mut world = server::start_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), &crate::worlds::types()).expect("the world started");
    let origin = origin(&world);
    let plant_all = |world: &mut World| {
        for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
            plant_grass(world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
        }
    };
    plant_all(&mut world);
    assert_eq!(cells_of_grass(&world), CELLS);
    let (ticks, mut decays_counted) = (seeded_random.draw_between(40, 80), 0);
    for _ in 0..ticks {
        let tick_counts = tick_grass(&mut world).rules;
        assert_eq!(tick_counts[SPREADS_COUNT], 0, "no dirt to spread onto but what decayed, and that is not spread onto in a tick");
        decays_counted += tick_counts[DECAYS_COUNT];
        // What decayed is dirt again, to be spread onto: put back, so every tick is of a superchunk all grass.
        if tick_counts[DECAYS_COUNT] > 0 {
            plant_all(&mut world);
        }
    }
    let expected_decays = ticks as f64 * CELLS as f64 * DECAY_CHANCE.as_fraction();
    assert!(expected_decays > 50.0 && (decays_counted as f64 - expected_decays).abs() <= 4.0 * expected_decays.sqrt(), "{decays_counted} decays in {ticks} ticks, about {expected_decays:.0} expected");
}

/// Lone cells of grass, each with dirt all round: none ever decays --
/// no grass crowds it -- and they spread at their chance, give or take
/// four times what chance alone moves that many by.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn lone_grass_never_decays_and_spreads_at_its_chance() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), &crate::worlds::types()).expect("the world started");
    let origin = origin(&world);
    // A cell every fourth across and down: far enough apart that one spread never makes two neighbours.
    for (x, y) in (0..256).flat_map(|y| (0..256).map(move |x| (x, y))) {
        plant_grass(&mut world, CellCartesian { x: origin.x + 4 * x + 1, y: origin.y + 4 * y + 1 }, 1, 1);
    }
    let lone_cells = cells_of_grass(&world);
    let (mut spreads_counted, mut ticks) = (0, 0);
    while spreads_counted < 400 {
        let tick_counts = tick_grass(&mut world).rules;
        // Until the first spread no grass has grass beside it.
        if spreads_counted == 0 {
            assert_eq!(tick_counts[DECAYS_COUNT], 0, "lone grass decayed");
        }
        spreads_counted += tick_counts[SPREADS_COUNT];
        ticks += 1;
        assert!(ticks < 100_000, "the grass never spreads");
    }
    let expected_spreads = ticks as f64 * lone_cells as f64 * SPREAD_CHANCE.as_fraction();
    assert!((spreads_counted as f64 - expected_spreads).abs() <= 4.0 * expected_spreads.sqrt() + 1.0, "{spreads_counted} spreads in {ticks} ticks, about {expected_spreads:.0} expected");
}

/// Over 1,000 ticks the grass changes by
/// no more than what spread and decayed, and scattered grass grows --
/// by no more than spreading alone would make of it.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_changes_by_what_spread_and_decayed() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    // Grass enough that some of it spreads in so few ticks.
    let mut world = server::start_world(&plain_start(1, 6_250, 0, 1).with(SEED, ParameterValue::Hex(seed)), &crate::worlds::types()).expect("the world started");
    let grass_at_start = cells_of_grass(&world);
    let mut grass_before = grass_at_start;
    for _ in 0..1000 {
        let tick_counts = tick_grass(&mut world).rules;
        let grass_now = cells_of_grass(&world);
        assert!(grass_now + tick_counts[DECAYS_COUNT] >= grass_before && grass_now + tick_counts[DECAYS_COUNT] <= grass_before + tick_counts[SPREADS_COUNT], "grown by what spread, less what decayed");
        grass_before = grass_now;
    }
    let grass_growth = grass_before as f64 / grass_at_start as f64;
    assert!(grass_growth > 1.0 && grass_growth < (1.0 + 2.0 * 1000.0 * SPREAD_CHANCE.as_fraction()) * 1.1, "grew {grass_growth:.2} times");
}
