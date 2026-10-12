//! Trees: a cell set in [`TREE`] with a stage in [`TREE_STAGE`], four
//! bits a cell. A tree sampled tries to spread, or grows a stage, or
//! at the oldest may die (`docs/sca_rules.md`, "Trees").

use crate::growing::{humid_enough_to_spread, too_harsh_to_live};
use actions::types::{COLLISION, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use actions::simulation_time::SimulationTime;
use actions::{cells, compare, place_counted, this_tick, CellIndex, Chance, RuleCounts, Turn};

/// The simulation time from one sample of a tree to the next, on
/// average: what its chance each tick is worked out from, so trees
/// grow as fast at any tick rate.
pub const MEAN_TIME_TO_SAMPLE: SimulationTime = SimulationTime::seconds(260);
/// The chance, each tick, that a tree is sampled.
pub const SAMPLE_CHANCE: Chance = MEAN_TIME_TO_SAMPLE.chance_a_tick();
/// The share of a tree's samples it tries to spread in; it grows in the rest.
pub const SPREAD_SHARE: Chance = Chance::HALF;
/// The rest: the share of its samples a tree grows in, or at the
/// oldest stage may die in.
pub const GROW_SHARE: Chance = Chance::HALF;
/// The stage from which a tree spreads.
pub const SEEDS_FROM: u32 = 4;
/// The other trees about a tree at which it never spreads.
pub const CROWDED: u32 = 9;
/// The simulation time a tree stands at the oldest stage before it
/// dies, on average.
pub const MEAN_TIME_TO_DIE: SimulationTime = SimulationTime::seconds(2_080);
/// The chance, each sample of a tree at the oldest stage, that it
/// dies: drawn among the samples it does not spread in.
pub const DIE_CHANCE: Chance = MEAN_TIME_TO_DIE.chance_each(MEAN_TIME_TO_SAMPLE);
/// Cells along the side of the square about a tree it counts and spreads in.
pub const AROUND: u32 = 8;

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 4] = ["sampled", "spreads", "grown", "died"];
/// Trees sampled.
pub const SAMPLED: usize = place_counted(&COUNTED, "sampled");
/// Trees put: counted as each is applied, so two put on one cell are
/// one.
pub const SPREADS: usize = place_counted(&COUNTED, "spreads");
/// Trees grown a stage: counted as each is applied.
pub const GROWN: usize = place_counted(&COUNTED, "grown");
/// Trees dead: counted as each is applied.
pub const DIED: usize = place_counted(&COUNTED, "died");

/// The rule, on one tree sampled -- each with [`SAMPLE_CHANCE`], by
/// whoever ticks the rule: it tries to spread, or it grows -- or
/// dies: of age at the oldest stage, with [`DIE_CHANCE`], and at any
/// stage of its climate, with that chance times how harsh its tile's
/// is ([`too_harsh_to_live`]): the hotter and drier, the sooner.
#[inline]
pub fn rule(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED] += 1;
    let spreading = this_tick::random(turn).chance(SPREAD_SHARE);
    let Some(stage) = cells::value(turn, TREE_STAGE, cell) else {
        return;
    };
    if spreading {
        spread(turn, cell, stage);
        return;
    }
    let of_age = stage == OLDEST_TREE_STAGE && this_tick::random(turn).chance_among(DIE_CHANCE, GROW_SHARE);
    if of_age || this_tick::random(turn).chance_among(DIE_CHANCE, GROW_SHARE) && too_harsh_to_live(turn, cell) {
        die(turn, cell, stage);
    } else if stage < OLDEST_TREE_STAGE {
        cells::set_value_counted(turn, TREE_STAGE, cell, stage, stage + 1, GROWN);
    }
}

/// The tree at `cell`, `stage` old, dies: counted as it is applied.
fn die(turn: &mut Turn, cell: CellIndex, stage: u32) {
    // Its stage goes with it, so the next tree there starts at 0: gone if the tree still stands, which is then cleared.
    compare::write(turn, compare::holds(TREE, cell), TREE_STAGE.layer_type(), cell, stage, 0, None);
    // And its cell is free to step on again, if it is the tree that held it.
    compare::write(turn, compare::holds(TREE, cell), COLLISION, cell, 1, 0, None);
    cells::clear_counted(turn, TREE, cell, DIED);
}

/// The tree at `cell`, `stage` old, tries to spread: one put where it
/// may -- no water, nothing standing, and the air humid enough this
/// time ([`humid_enough_to_spread`]) -- is counted as it is applied.
fn spread(turn: &mut Turn, cell: CellIndex, stage: u32) {
    if stage < SEEDS_FROM {
        return;
    }
    let Some((corner, around)) = cells::square(turn, TREE, cell, AROUND) else {
        return;
    };
    // Itself is one of those counted.
    let others = around.set.count_ones().saturating_sub(1);
    // The more trees about it, the less likely: never with as many as crowd it.
    if this_tick::random(turn).below(CROWDED as u64) < others as u64 {
        return;
    }
    let drawn = this_tick::random(turn).below((AROUND * AROUND) as u64) as u32;
    let free = around.hot >> drawn & 1 == 1 && around.set >> drawn & 1 == 0;
    let Some(onto) = corner.offset((drawn % AROUND) as i32, (drawn / AROUND) as i32).filter(|_| free) else {
        return;
    };
    // No tree under water, nor where something stands already; and only as often as the air there is humid.
    if cells::holds(turn, WET, onto) || cells::holds(turn, COLLISION, onto) || !humid_enough_to_spread(turn, onto) {
        return;
    }
    // The tree is put only if its cell is still free in the collision plane -- of two trees, or a tree and a sheep stepping there, the
    // first applied has it -- and then holds it: where a tree stands is not stepped on.
    compare::write(turn, compare::lacks(COLLISION, onto), TREE, onto, 0, 1, Some(SPREADS));
    cells::set(turn, COLLISION, onto);
}
