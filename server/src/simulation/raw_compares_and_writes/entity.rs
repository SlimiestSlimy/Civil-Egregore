//! Writes of an entity whole: made, moved -- within its superchunk
//! or over a border -- removed: each one instruction
//! (`docs/compares_and_writes.md`, "Writes").

use utilities::coordinates::CellIndex;
use crate::entity_manager::Header;
use crate::simulation::{SimulationTickState, Write};

/// Queues an entity made as `header` says: of its type, on its cell,
/// to wake at its wake.
#[inline]
pub fn queue_entity_made(tick_state: &mut SimulationTickState, header: Header) {
    tick_state.write(Write::MakeEntity(header));
}

/// Queues the entity the tick found on `from_cell` moved to `to_cell`, a cell
/// of its superchunk or of another.
#[inline]
pub fn move_to(tick_state: &mut SimulationTickState, from_cell: CellIndex, to_cell: CellIndex) {
    tick_state.write(Write::MoveEntity { from_cell, to_cell });
}

/// Queues the entity the tick found on `entity_cell` removed.
#[inline]
pub fn remove(tick_state: &mut SimulationTickState, entity_cell: CellIndex) {
    tick_state.write(Write::RemoveEntity(entity_cell));
}
