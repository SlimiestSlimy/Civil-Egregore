//! Trees, on a plain the server makes: they spread once old enough and
//! not where they are crowded, and die of age only at the oldest. How
//! they grow, read every tick, is the complete tier's (`trees.rs`).

use crate::tree_planting::{bare_world, plant_trees};
use crate::worlds::tick_rule;
use server::simulation::simulation_api::CellIndex;
use basegame::base_types::{OLDEST_TREE_STAGE, TREES_RULE};
use basegame::sca_rules::trees::{DIED_COUNT, SAMPLED_COUNT, SEEDS_FROM, SPREADS_COUNT};
use utilities::rng::RandomSource;

/// Trees on every cell: none ever spreads, however old -- each has
/// more about it than crowd it, and stays so as some die.
#[test]
fn crowded_trees_never_spread() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe, every_cell) = bare_world("crowded_trees_never_spread", seed);
    plant_trees(&mut world, &probe, &every_cell, || OLDEST_TREE_STAGE);
    let (mut sampled_total, mut died_total) = (0, 0);
    for tick in 0..300 {
        let tick_counts = tick_rule(&mut world, TREES_RULE).rules;
        assert_eq!(tick_counts[SPREADS_COUNT], 0, "tick {tick}");
        (sampled_total, died_total) = (sampled_total + tick_counts[SAMPLED_COUNT], died_total + tick_counts[DIED_COUNT]);
    }
    assert!(sampled_total > 1_000 && died_total > 100, "{sampled_total} sampled, {died_total} died");
}

/// Young trees scattered thin, old enough to spread and none at the
/// oldest stage: they spread, and in too few ticks for any to grow to
/// the oldest, none dies.
#[test]
fn young_trees_spread_and_none_dies() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let (mut world, probe, cells) = bare_world("young_trees_spread_and_none_dies", seed);
    let planted_cells: Vec<CellIndex> = cells.iter().copied().filter(|_| seeded_random.draw_below(16) == 0).collect();
    plant_trees(&mut world, &probe, &planted_cells, || SEEDS_FROM);
    // Too few ticks for a tree to grow from there to the oldest stage.
    let ticks = 700;
    assert!(u64::from(OLDEST_TREE_STAGE - SEEDS_FROM) * basegame::sca_rules::trees::MEAN_TIME_TO_SAMPLE.ticks() > 8 * ticks);
    let (spread, died) = (0..ticks).map(|_| tick_rule(&mut world, TREES_RULE).rules).fold((0, 0), |spread_died_totals, tick_counts| (spread_died_totals.0 + tick_counts[SPREADS_COUNT], spread_died_totals.1 + tick_counts[DIED_COUNT]));
    assert!(spread > 20 && died == 0, "{spread} spread and {died} died");
}
