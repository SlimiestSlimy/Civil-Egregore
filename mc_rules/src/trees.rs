//! Trees: a rule of the cells with more than a bit a cell. A tree is a
//! cell set in [`TREE`], and has a stage, 0 to [`OLDEST`], kept in
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

use instructions::layers::WET;
use instructions::{read, write, Bits4, CellIndex, LayerType, Turn, Wide};
use std::ops::AddAssign;

/// The cells a tree stands on.
pub const TREE: LayerType = LayerType(3);
/// A tree's stage: a plane four bits a cell wide, kept cold as the
/// four layer types from 4 on, a bit each.
pub const TREE_STAGE: Wide<Bits4> = Wide::new(4);
/// The oldest stage: sixteen in all.
pub const OLDEST: u32 = 15;

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

/// What the rule did in a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeCounts {
    /// Trees sampled.
    pub sampled: usize,
    /// Trees put: two may be put on one cell, which then has one.
    pub spreads: usize,
    /// Trees grown a stage.
    pub grown: usize,
    /// Trees dead.
    pub died: usize,
}

impl AddAssign for TreeCounts {
    /// All added up.
    fn add_assign(&mut self, other: Self) {
        self.sampled += other.sampled;
        self.spreads += other.spreads;
        self.grown += other.grown;
        self.died += other.died;
    }
}

/// The rule, on one superchunk's turn: every tree sampled with
/// [`SAMPLE_CHANCE`], in Morton order, each seen to by [`tree`].
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> TreeCounts {
    let (sampled, counts) = read::cells::each_sampled(turn, TREE, SAMPLE_CHANCE, samples, tree);
    TreeCounts { sampled, ..counts }
}

/// The rule, on one tree sampled: it tries to spread or grows -- or, at
/// the oldest stage, may die.
#[inline]
fn tree(turn: &mut Turn, cell: CellIndex, counts: &mut TreeCounts) {
    let spreading = turn.random().unit() <= SPREAD_SHARE;
    let Some(stage) = read::cells::value(turn, TREE_STAGE, cell) else {
        return;
    };
    if spreading {
        counts.spreads += spread(turn, cell, stage) as usize;
    } else if stage < OLDEST {
        write::cells::set_value(turn, TREE_STAGE, cell, stage + 1);
        counts.grown += 1;
    } else if turn.random().below(DIE_ONE_IN) == 0 {
        write::cells::clear(turn, TREE, cell);
        // Its stage goes with it: the next tree there starts at 0.
        write::cells::set_value(turn, TREE_STAGE, cell, 0);
        counts.died += 1;
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
