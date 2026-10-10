//! Trees: a cell set in [`TREE`] with a stage in [`TREE_STAGE`], four
//! bits a cell. A tree sampled tries to spread, or grows a stage, or
//! at the oldest may die (`docs/mc_rules.md`, "Trees").

use instructions::layers::{OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use instructions::{cells, CellIndex, Chance, RuleCounts, Turn};

/// The chance, each tick, that a tree is sampled.
pub const SAMPLE_CHANCE: Chance = Chance::one_in(10_000);
/// The share of a tree's samples it tries to spread in; it grows in the rest.
pub const SPREAD_SHARE: Chance = Chance::HALF;
/// The stage from which a tree spreads.
pub const SEEDS_FROM: u32 = 4;
/// The other trees about a tree at which it never spreads.
pub const CROWDED: u32 = 9;
/// A tree at the oldest stage dies one sample in this many of those it does not spread in.
pub const DIE_ONE_IN: u64 = 4;
/// Cells along the side of the square about a tree it counts and spreads in.
pub const AROUND: u32 = 8;

/// What the rule counts, each named at its place in its [`RuleCounts`].
pub const COUNTED: [&str; 4] = ["sampled", "spreads", "grown", "died"];
/// Trees sampled.
pub const SAMPLED: usize = 0;
/// Trees put: two may be put on one cell, which then has one.
pub const SPREADS: usize = 1;
/// Trees grown a stage.
pub const GROWN: usize = 2;
/// Trees dead.
pub const DIED: usize = 3;

/// The rule, on one superchunk's turn: every tree sampled with
/// [`SAMPLE_CHANCE`], in Morton order, each seen to by [`tree`].
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> RuleCounts {
    let (sampled, mut counts) = cells::each_sampled(turn, TREE, SAMPLE_CHANCE, samples, tree);
    counts[SAMPLED] = sampled as u64;
    counts
}

/// The rule, on one tree sampled: it tries to spread or grows -- or, at
/// the oldest stage, may die.
#[inline]
fn tree(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    let spreading = turn.random().chance(SPREAD_SHARE);
    let Some(stage) = cells::value(turn, TREE_STAGE, cell) else {
        return;
    };
    if spreading {
        counts[SPREADS] += u64::from(spread(turn, cell, stage));
    } else if stage < OLDEST_TREE_STAGE {
        cells::set_value(turn, TREE_STAGE, cell, stage + 1);
        counts[GROWN] += 1;
    } else if turn.random().below(DIE_ONE_IN) == 0 {
        cells::clear(turn, TREE, cell);
        // Its stage goes with it: the next tree there starts at 0.
        cells::set_value(turn, TREE_STAGE, cell, 0);
        counts[DIED] += 1;
    }
}

/// The tree at `cell`, `stage` old, tries to spread: whether it put one.
fn spread(turn: &mut Turn, cell: CellIndex, stage: u32) -> bool {
    if stage < SEEDS_FROM {
        return false;
    }
    let Some((corner, around)) = cells::square(turn, TREE, cell, AROUND) else {
        return false;
    };
    // Itself is one of those counted.
    let others = around.set.count_ones().saturating_sub(1);
    // The more trees about it, the less likely: never with as many as crowd it.
    if turn.random().below(CROWDED as u64) < others as u64 {
        return false;
    }
    let drawn = turn.random().below((AROUND * AROUND) as u64) as u32;
    let free = around.hot >> drawn & 1 == 1 && around.set >> drawn & 1 == 0;
    let Some(onto) = corner.offset((drawn % AROUND) as i32, (drawn / AROUND) as i32).filter(|_| free) else {
        return false;
    };
    // No tree under water.
    if cells::holds(turn, WET, onto) {
        return false;
    }
    cells::set(turn, TREE, onto);
    true
}
