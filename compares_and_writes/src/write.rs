//! Writes: a cell, an entity's attribute or wake, a count -- each one
//! instruction, applied only if every compare open as it was queued
//! holds when it is come to (`docs/compares_and_writes.md`, "Writes").

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeBlock, AttributeType, Member};
use simulation::{Turn, Write};

/// Queues the number of `layer_type` at `cell` becoming `value` -- 1
/// or 0 on a layer of a bit a cell: one added to the rule's count
/// `count_index` as it is applied, if one is given.
#[inline]
pub fn cell(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, value: u32, count_index: Option<usize>) {
    turn.write(Write::Cell { layer_type, target_cell: cell, value, counted_place: count_index.map(|place| place as u32) });
}

/// Queues one added to the rule's count `count_index`.
#[inline]
pub fn count(turn: &mut Turn, count_index: usize) {
    turn.write(Write::Count(count_index as u32));
}

/// Queues `member` of the attribute of type `attribute_type` of the entity the
/// tick found on `entity_cell` becoming `value`.
#[inline]
pub fn attribute_member(turn: &mut Turn, entity_cell: CellIndex, attribute_type: AttributeType, member: Member, value: u64) {
    turn.write(Write::AttributeMember { named_cell: entity_cell, attribute_type, member, value });
}

/// Queues the entity the tick found on `entity_cell`, or made on it under the
/// compare open, given `attribute` whole: the blocks of one
/// attribute.
#[inline]
pub fn attribute_whole(turn: &mut Turn, entity_cell: CellIndex, attribute: &[AttributeBlock]) {
    turn.write_attribute(entity_cell, attribute);
}

/// Queues the attribute of type `attribute_type` taken from the entity the tick
/// found on `entity_cell`.
#[inline]
pub fn attribute_removed(turn: &mut Turn, entity_cell: CellIndex, attribute_type: AttributeType) {
    turn.write(Write::RemoveAttribute { named_cell: entity_cell, attribute_type });
}

/// Queues the entity the tick found on `entity_cell` to wake next at `wake`.
#[inline]
pub fn wake(turn: &mut Turn, entity_cell: CellIndex, wake: u64) {
    turn.write(Write::Wake { named_cell: entity_cell, wake });
}
