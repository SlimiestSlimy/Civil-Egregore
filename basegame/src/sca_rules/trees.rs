//! Trees: a cell set in the tree layer with a stage in the tree stage plane, four
//! bits a cell. A tree sampled tries to spread, or grows a stage, or
//! at the oldest may die (`docs/sca_rules.md`, "Trees").

use crate::base_types::{BaseTypes, OLDEST_TREE_STAGE};
use protocol::server_types::simulation_time::SimulationTime;
use server::simulation::simulation_api::{on_each_sampled, RuleCode, cell_reads_and_writes, compare_and_write, tick_and_random_stream, CellIndex, Chance, SimulationTickState};
use server::simulation::simulation_api::rule_counts::{place_counted, RuleCounts};

/// The simulation time from one sample of a tree to the next, on
/// average: what its chance each tick is worked out from, so trees
/// grow as fast at any tick rate.
pub const MEAN_TIME_TO_SAMPLE: SimulationTime = SimulationTime::from_seconds(260);
/// The chance, each tick, that a tree is sampled.
pub const SAMPLE_CHANCE: Chance = MEAN_TIME_TO_SAMPLE.chance_a_tick();
/// The share of a tree's samples it tries to spread in; it grows in the rest.
pub const SPREAD_SHARE: Chance = Chance::HALF_THE_TIME;
/// The rest: the share of its samples a tree grows in, or at the
/// oldest stage may die in.
pub const GROW_SHARE: Chance = Chance::HALF_THE_TIME;
/// The stage from which a tree spreads.
pub const SEEDS_FROM: u32 = 4;
/// The other trees about a tree at which it never spreads.
pub const CROWDED_AT: u32 = 9;
/// The simulation time a tree stands at the oldest stage before it
/// dies, on average.
pub const MEAN_TIME_TO_DIE: SimulationTime = SimulationTime::from_seconds(2_080);
/// The chance, each sample of a tree at the oldest stage, that it
/// dies: drawn among the samples it does not spread in.
pub const DIE_CHANCE: Chance = MEAN_TIME_TO_DIE.chance_each(MEAN_TIME_TO_SAMPLE);
/// Cells along the side of the square about a tree it counts and spreads in.
pub const AROUND: u32 = 8;

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNT_NAMES: [&str; 4] = ["sampled", "spreads", "grown", "died"];
/// What runs the trees' rule, as it is registered with `base_types`:
/// [`rule`] on each sample of its master, chosen with [`SAMPLE_CHANCE`].
pub fn code(base_types: BaseTypes) -> RuleCode {
    RuleCode::new(&COUNT_NAMES, move |tick_state, context| on_each_sampled(tick_state, context.master, SAMPLE_CHANCE, |tick_state, cell, counts| rule(&base_types, tick_state, cell, counts)))
}
/// Trees sampled.
pub const SAMPLED_COUNT: usize = place_counted(&COUNT_NAMES, "sampled");
/// Trees put: counted as each is applied, so two put on one cell are
/// one.
pub const SPREADS_COUNT: usize = place_counted(&COUNT_NAMES, "spreads");
/// Trees grown a stage: counted as each is applied.
pub const GROWN_COUNT: usize = place_counted(&COUNT_NAMES, "grown");
/// Trees dead: counted as each is applied.
pub const DIED_COUNT: usize = place_counted(&COUNT_NAMES, "died");

/// The rule, on one tree sampled -- each with [`SAMPLE_CHANCE`], by
/// whoever ticks the rule: it tries to spread, or it grows -- or, at
/// the oldest stage, dies of age with [`DIE_CHANCE`].
#[inline]
pub fn rule(base_types: &BaseTypes, tick_state: &mut SimulationTickState, cell: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED_COUNT] += 1;
    let spreads_this_time = tick_and_random_stream::random_stream(tick_state).chance(SPREAD_SHARE);
    let Some(stage) = cell_reads_and_writes::value(tick_state, base_types.tree_stage, cell) else {
        return;
    };
    if spreads_this_time {
        tree_tries_to_spread(base_types, tick_state, cell, stage);
        return;
    }
    let of_age = stage == OLDEST_TREE_STAGE && tick_and_random_stream::random_stream(tick_state).chance_among(DIE_CHANCE, GROW_SHARE);
    if of_age {
        tree_dies(base_types, tick_state, cell, stage);
    } else if stage < OLDEST_TREE_STAGE {
        cell_reads_and_writes::set_value_counted(tick_state, base_types.tree_stage, cell, stage, stage + 1, GROWN_COUNT);
    }
}

/// The tree at `cell`, `stage` old, dies: counted as it is applied.
fn tree_dies(base_types: &BaseTypes, tick_state: &mut SimulationTickState, cell: CellIndex, stage: u32) {
    // Its stage goes with it, so the next tree there starts at 0: gone if the tree still stands, which is then cleared.
    compare_and_write::write(tick_state, compare_and_write::layer_holds(base_types.tree, cell), base_types.tree_stage.layer_type(), cell, stage, 0, None);
    // And its cell is free to step on again, if it is the tree that held it.
    compare_and_write::write(tick_state, compare_and_write::layer_holds(base_types.tree, cell), base_types.collision, cell, 1, 0, None);
    cell_reads_and_writes::clear_counted(tick_state, base_types.tree, cell, DIED_COUNT);
}

/// The tree at `cell`, `stage` old, tries to spread: one put where it
/// may -- no water, nothing standing -- is counted as it is applied.
fn tree_tries_to_spread(base_types: &BaseTypes, tick_state: &mut SimulationTickState, cell: CellIndex, stage: u32) {
    if stage < SEEDS_FROM {
        return;
    }
    let Some((window_corner, around)) = cell_reads_and_writes::window_about(tick_state, base_types.tree, cell, AROUND) else {
        return;
    };
    // Itself is one of those counted.
    let other_trees = around.cells_set.count_ones().saturating_sub(1);
    // The more trees about it, the less likely: never with as many as crowd it.
    if tick_and_random_stream::random_stream(tick_state).draw_below(CROWDED_AT as u64) < other_trees as u64 {
        return;
    }
    let drawn_place = tick_and_random_stream::random_stream(tick_state).draw_below((AROUND * AROUND) as u64) as u32;
    let drawn_free = around.hot >> drawn_place & 1 == 1 && around.cells_set >> drawn_place & 1 == 0;
    let Some(onto_cell) = window_corner.at_cartesian_offset((drawn_place % AROUND) as i32, (drawn_place / AROUND) as i32).filter(|_| drawn_free) else {
        return;
    };
    // No tree under water, nor where something stands already.
    if cell_reads_and_writes::layer_holds(tick_state, base_types.wet, onto_cell) || cell_reads_and_writes::layer_holds(tick_state, base_types.collision, onto_cell) {
        return;
    }
    // The tree is put only if its cell is still free in the collision plane -- of two trees, or a tree and a sheep stepping there, the
    // first applied has it -- and then holds it: where a tree stands is not stepped on.
    compare_and_write::write(tick_state, compare_and_write::layer_lacks(base_types.collision, onto_cell), base_types.tree, onto_cell, 0, 1, Some(SPREADS_COUNT));
    cell_reads_and_writes::set(tick_state, base_types.collision, onto_cell);
}
