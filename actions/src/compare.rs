//! Compares, and what is made of one and a write: a compare is an
//! instruction (`../compares_and_writes/`), handed on here as a rule asks
//! for it; a write under a compare of something else, and a count
//! under one, are several (`docs/actions.md`, "Compare-and-write").

use chunk_storage::LayerType;
use coordinates::CellIndex;
use compares_and_writes::compare::{close_compare, open_compare};
pub use compares_and_writes::compare::{attribute, end_from_here, from_here, layer_holds, layer_lacks, value, Compare};
use simulation::Turn;

/// Queues the number of `layer_type` at `cell` becoming `value` if
/// it is still the `value_seen` the rule read there and `under_compare` holds too
/// when the write is come to: one added to the rule's count `count_place`
/// then, if one is given. The compare is of the cell's superchunk.
#[inline]
pub fn write(turn: &mut Turn, under_compare: Compare, layer_type: LayerType, cell: CellIndex, value_seen: u32, value: u32, count_place: Option<usize>) {
    open_compare(turn, under_compare);
    crate::cells::seen_becoming(turn, layer_type, cell, value_seen, value, count_place);
    close_compare(turn);
}

/// Adds one to the rule's count `count_place` if `under_compare` holds when it
/// is come to.
#[inline]
pub fn count(turn: &mut Turn, under_compare: Compare, count_place: usize) {
    compares_and_writes::compare::with_writes(turn, under_compare, &[compares_and_writes::Write::Count(count_place as u32)]);
}
