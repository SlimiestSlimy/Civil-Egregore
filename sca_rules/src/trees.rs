//! Trees: a cell set in [`TREE`] with a stage in [`TREE_STAGE`], four
//! bits a cell. A tree sampled tries to spread, or grows a stage, or
//! at the oldest may die (`docs/sca_rules.md`, "Trees").

use instructions::layers::{OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use instructions::{cells, compare, place_counted, CellIndex, Chance, RuleCounts, Turn};

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
fn tree(turn: &mut Turn, cell: CellIndex, _counts: &mut RuleCounts) {
    let spreading = turn.random().chance(SPREAD_SHARE);
    let Some(stage) = cells::value(turn, TREE_STAGE, cell) else {
        return;
    };
    if spreading {
        spread(turn, cell, stage);
    } else if stage < OLDEST_TREE_STAGE {
        cells::set_value_counted(turn, TREE_STAGE, cell, stage, stage + 1, GROWN);
    } else if turn.random().below(DIE_ONE_IN) == 0 {
        // Its stage goes with it, so the next tree there starts at 0: gone if the tree still stands, which is then cleared.
        compare::write(turn, compare::holds(TREE, cell), TREE_STAGE.layer_type(), cell, stage, 0, None);
        cells::clear_counted(turn, TREE, cell, DIED);
    }
}

/// The tree at `cell`, `stage` old, tries to spread: one put where it
/// may is counted as it is applied.
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
    if turn.random().below(CROWDED as u64) < others as u64 {
        return;
    }
    let drawn = turn.random().below((AROUND * AROUND) as u64) as u32;
    let free = around.hot >> drawn & 1 == 1 && around.set >> drawn & 1 == 0;
    let Some(onto) = corner.offset((drawn % AROUND) as i32, (drawn / AROUND) as i32).filter(|_| free) else {
        return;
    };
    // No tree under water.
    if cells::holds(turn, WET, onto) {
        return;
    }
    cells::set_counted(turn, TREE, onto, SPREADS);
}
