//! Writes of an entity whole: made, moved -- within its superchunk
//! or over a border -- removed: each one instruction
//! (`docs/compares_and_writes.md`, "Writes").

use coordinates::CellIndex;
use entity_manager::Header;
use simulation::{Turn, Write};

/// Queues an entity made as `header` says: of its type, on its cell,
/// to wake at its wake.
#[inline]
pub fn queue_entity_made(turn: &mut Turn, header: Header) {
    turn.write(Write::MakeEntity(header));
}

/// Queues the entity the tick found on `from_cell` moved to `to_cell`, a cell
/// of its superchunk or of another.
#[inline]
pub fn move_to(turn: &mut Turn, from_cell: CellIndex, to_cell: CellIndex) {
    turn.write(Write::MoveEntity { from_cell, to_cell });
}

/// Queues the entity the tick found on `entity_cell` removed.
#[inline]
pub fn remove(turn: &mut Turn, entity_cell: CellIndex) {
    turn.write(Write::RemoveEntity(entity_cell));
}
