//! Trees: a rule of the cells with more than a bit a cell. A tree is a
//! cell set in [`TREE`], and has a stage, 0 to [`OLDEST_TREE_STAGE`], kept in
//! [`TREE_STAGE`], a plane four bits a cell wide: a cell's stage is one
//! read and one write ([`read::cells::value`]).
//!
//! Each tick every tree is sampled with [`SAMPLE_CHANCE`], and a tree
//! sampled does one thing, by lot:
//!
//! - **It tries to spread**, in [`SPREAD_SHARE`] of its samples, if it
//!   is at least [`SEEDS_FROM`] old. It counts the other trees in the 8
//!   by 8 cells about it: the more there are the less likely it
//!   spreads, never with [`CROWDED`] or more. If it does, a cell of
//!   those 64 is drawn, and a tree of stage 0 is put there if there is
//!   none and the cell is not under water.
//! - **Else it grows** a stage; or, at the oldest stage, dies one time
//!   in [`DIE_ONE_IN`] -- the cell cleared, and its stage -- and lives
//!   on otherwise.
//!
//! Trees stand on dirt and grass alike and change neither.

use instructions::layers::{OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use instructions::{read, write, CellIndex, RuleCounts, Turn};

/// The chance, each tick, that a tree is sampled.
pub const SAMPLE_CHANCE: f64 = 0.000_1;
/// The share of a tree's samples it tries to spread in; it grows in the rest.
pub const SPREAD_SHARE: f64 = 0.5;
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
    let (sampled, mut counts) = read::cells::each_sampled(turn, TREE, SAMPLE_CHANCE, samples, tree);
    counts[SAMPLED] = sampled as u64;
    counts
}

/// The rule, on one tree sampled: it tries to spread or grows -- or, at
/// the oldest stage, may die.
#[inline]
fn tree(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    let spreading = turn.random().unit() <= SPREAD_SHARE;
    let Some(stage) = read::cells::value(turn, TREE_STAGE, cell) else {
        return;
    };
    if spreading {
        counts[SPREADS] += u64::from(spread(turn, cell, stage));
    } else if stage < OLDEST_TREE_STAGE {
        write::cells::set_value(turn, TREE_STAGE, cell, stage + 1);
        counts[GROWN] += 1;
    } else if turn.random().below(DIE_ONE_IN) == 0 {
        write::cells::clear(turn, TREE, cell);
        // Its stage goes with it: the next tree there starts at 0.
        write::cells::set_value(turn, TREE_STAGE, cell, 0);
        counts[DIED] += 1;
    }
}

/// The tree at `cell`, `stage` old, tries to spread: whether it put one.
fn spread(turn: &mut Turn, cell: CellIndex, stage: u32) -> bool {
    if stage < SEEDS_FROM {
        return false;
    }
    let Some((corner, around)) = read::cells::square(turn, TREE, cell, AROUND) else {
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
    if read::cells::holds(turn, WET, onto) {
        return false;
    }
    write::cells::set(turn, TREE, onto);
    true
}
