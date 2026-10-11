//! Writes: a cell, an entity's attribute or wake, a count -- each one
//! instruction, applied only if every compare open as it was queued
//! holds when it is come to (`docs/compares_and_writes.md`, "Writes").

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeBlock, AttributeType, Member};
use simulation::Turn;

/// Queues the number of `layer_type` at `cell` becoming `value` -- 1
/// or 0 on a layer of a bit a cell: one added to the rule's count
/// `counted` as it is applied, if one is given.
#[inline]
pub fn cell(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, value: u32, counted: Option<usize>) {
    turn.write_cell(layer_type, cell, value, counted.map(|place| place as u32));
}

/// Queues one added to the rule's count `counted`.
#[inline]
pub fn count(turn: &mut Turn, counted: usize) {
    turn.count(counted as u32);
}

/// Queues `member` of the attribute of type `kind` of the entity the
/// tick found on `at` becoming `value`.
#[inline]
pub fn attribute_member(turn: &mut Turn, at: CellIndex, kind: AttributeType, member: Member, value: u64) {
    turn.write_attribute_member(at, kind, member, value);
}

/// Queues the entity the tick found on `at`, or made on it under the
/// compare open, given `attribute` whole: the blocks of one
/// attribute.
#[inline]
pub fn attribute_whole(turn: &mut Turn, at: CellIndex, attribute: &[AttributeBlock]) {
    turn.write_attribute(at, attribute);
}

/// Queues the attribute of type `kind` taken from the entity the tick
/// found on `at`.
#[inline]
pub fn attribute_removed(turn: &mut Turn, at: CellIndex, kind: AttributeType) {
    turn.remove_attribute(at, kind);
}

/// Queues the entity the tick found on `at` to wake next at `wake`.
#[inline]
pub fn wake(turn: &mut Turn, at: CellIndex, wake: u64) {
    turn.write_wake(at, wake);
}
