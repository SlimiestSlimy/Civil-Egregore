//! Grass over dirt, on a plain the server makes, by itself: it spreads
//! onto dirt at its chance, and turns back to dirt only where grass
//! crowds it.

use protocol::server_parameters::{ParameterValue, SEED};
use crate::probe::Probe;
use crate::worlds::{base, cells_of_grass, first_superchunk, nothing_more, plain_start, probed_world, set_rectangle, tick_grass};
use utilities::coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use basegame::sca_rules::grass::{DECAYS_COUNT, DECAY_CHANCE, SPREADS_COUNT, SPREAD_CHANCE};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream};
use server::World;
use utilities::rng::RandomSource;

/// Cells in a superchunk.
const CELLS: u64 = 1 << 20;

/// The first cell of `world`, at its top left, read in a tick of the probe alone.
fn origin(world: &mut World, probe: &Probe) -> CellCartesian {
    first_superchunk(world, probe).top_left().cartesian()
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
    let (mut world, probe) = probed_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let origin = origin(&mut world, &probe);
    // Every cell, in a tick of the probe alone: the grass rule's ticks are of a superchunk all grass.
    let plant_all = |world: &mut World| set_rectangle(world, &probe, base().grass, origin, SUPERCHUNK_SIDE_CELLS, SUPERCHUNK_SIDE_CELLS);
    plant_all(&mut world);
    assert_eq!(cells_of_grass(&mut world, &probe), CELLS);
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
    let (mut world, probe) = probed_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let origin = origin(&mut world, &probe);
    // A cell every fourth across and down: far enough apart that one spread never makes two neighbours.
    let lone_cells: Vec<CellIndex> = (0..256).flat_map(|y| (0..256).map(move |x| CellCartesian { x: origin.x + 4 * x + 1, y: origin.y + 4 * y + 1 }.into())).collect();
    probe.alone(&mut world, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for &lone_cell in lone_cells.iter().filter(|&&lone_cell| lone_cell.superchunk() == turn_superchunk) {
            cell_reads_and_writes::set(tick_state, base().grass, lone_cell);
        }
        Vec::<()>::new()
    });
    let lone_cells = cells_of_grass(&mut world, &probe);
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
    let (mut world, probe) = probed_world(&plain_start(1, 6_250, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let grass_at_start = cells_of_grass(&mut world, &probe);
    let mut grass_before = grass_at_start;
    for _ in 0..1000 {
        let tick_counts = tick_grass(&mut world).rules;
        // Counted in a tick of the probe alone, the grass's rule not run in it.
        let grass_now = cells_of_grass(&mut world, &probe);
        assert!(grass_now + tick_counts[DECAYS_COUNT] >= grass_before && grass_now + tick_counts[DECAYS_COUNT] <= grass_before + tick_counts[SPREADS_COUNT], "grown by what spread, less what decayed");
        grass_before = grass_now;
    }
    let grass_growth = grass_before as f64 / grass_at_start as f64;
    assert!(grass_growth > 1.0 && grass_growth < (1.0 + 2.0 * 1000.0 * SPREAD_CHANCE.as_fraction()) * 1.1, "grew {grass_growth:.2} times");
}
