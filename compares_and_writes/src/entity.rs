//! Writes of an entity whole: made, moved -- within its superchunk
//! or over a border -- removed: each one instruction
//! (`docs/compares_and_writes.md`, "Writes").

use coordinates::CellIndex;
use entity_manager::Header;
use simulation::Turn;

/// Queues an entity made as `header` says: of its type, on its cell,
/// to wake at its wake.
#[inline]
pub fn make(turn: &mut Turn, header: Header) {
    turn.make_entity(header);
}

/// Queues the entity the tick found on `from` moved to `to`, a cell
/// of its superchunk or of another.
#[inline]
pub fn move_to(turn: &mut Turn, from: CellIndex, to: CellIndex) {
    turn.move_entity(from, to);
}

/// Queues the entity the tick found on `at` removed.
#[inline]
pub fn remove(turn: &mut Turn, at: CellIndex) {
    turn.remove_entity(at);
}
