//! Writes: a cell, an entity's attribute or wake, a count -- each one
//! instruction, applied only if every compare open as it was queued
//! holds when it is come to (`docs/compares_and_writes.md`, "Writes").

use crate::chunk_storage::LayerType;
use utilities::coordinates::CellIndex;
use crate::entity_manager::AttributeType;
use protocol::server_types::attribute_layouts::Member;
use protocol::server_types::attributes::AttributeBlock;
use crate::simulation::{SimulationTickState, Write};

/// Queues the number of `layer_type` at `cell` becoming `value` -- 1
/// or 0 on a layer of a bit a cell: one added to the rule's count
/// `count_index` as it is applied, if one is given.
#[inline]
pub fn cell(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex, value: u32, count_index: Option<usize>) {
    tick_state.write(Write::Cell { layer_type, target_cell: cell, value, counted_place: count_index.map(|place| place as u32) });
}

/// Queues one added to the rule's count `count_index`.
#[inline]
pub fn count(tick_state: &mut SimulationTickState, count_index: usize) {
    tick_state.write(Write::Count(count_index as u32));
}

/// Queues `member` of the attribute of type `attribute_type` of the entity the
/// tick found on `entity_cell` becoming `value`.
#[inline]
pub fn attribute_member(tick_state: &mut SimulationTickState, entity_cell: CellIndex, attribute_type: AttributeType, member: Member, value: u64) {
    tick_state.write(Write::AttributeMember { named_cell: entity_cell, attribute_type, member, value });
}

/// Queues the entity the tick found on `entity_cell`, or made on it under the
/// compare open, given `attribute` whole: the blocks of one
/// attribute.
#[inline]
pub fn attribute_whole(tick_state: &mut SimulationTickState, entity_cell: CellIndex, attribute: &[AttributeBlock]) {
    tick_state.write_attribute(entity_cell, attribute);
}

/// Queues the attribute of type `attribute_type` taken from the entity the tick
/// found on `entity_cell`.
#[inline]
pub fn attribute_removed(tick_state: &mut SimulationTickState, entity_cell: CellIndex, attribute_type: AttributeType) {
    tick_state.write(Write::RemoveAttribute { named_cell: entity_cell, attribute_type });
}

/// Queues the entity the tick found on `entity_cell` to wake next at `wake`.
#[inline]
pub fn wake(tick_state: &mut SimulationTickState, entity_cell: CellIndex, wake: u64) {
    tick_state.write(Write::Wake { named_cell: entity_cell, wake });
}
