//! Compares, and what is made of one and a write: a compare is an
//! instruction (`../instructions/`), handed on here as a rule asks
//! for it; a write under a compare of something else, and a count
//! under one, are several (`docs/actions.md`, "Compare-and-write").

use chunk_storage::LayerType;
use coordinates::CellIndex;
use instructions::compare::{close, open};
pub use instructions::compare::{attribute, end, from_here, holds, lacks, value, Compare};
use simulation::Turn;

/// Queues the number of `layer_type` at `cell` becoming `value` if
/// it is still the `seen` the rule read there and `compare` holds too
/// when the write is come to: one added to the rule's count `counted`
/// then, if one is given. The compare is of the cell's superchunk.
#[inline]
pub fn write(turn: &mut Turn, compare: Compare, layer_type: LayerType, cell: CellIndex, seen: u32, value: u32, counted: Option<usize>) {
    open(turn, compare);
    crate::cells::seen_becoming(turn, layer_type, cell, seen, value, counted);
    close(turn);
}

/// Adds one to the rule's count `counted` if `compare` holds when it
/// is come to.
#[inline]
pub fn count(turn: &mut Turn, compare: Compare, counted: usize) {
    instructions::compare::with_writes(turn, compare, &[instructions::Write::Count(counted as u32)]);
}
