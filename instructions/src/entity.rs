//! Writes of an entity whole: made, moved, removed, crossing to
//! another superchunk -- each one instruction
//! (`docs/instructions.md`, "Writes").

use coordinates::CellIndex;
use entity_manager::{AttributeBlock, Header};
use simulation::Turn;

/// Queues an entity made as `header` says: of its type, on its cell,
/// to wake at its wake.
#[inline]
pub fn make(turn: &mut Turn, header: Header) {
    turn.make_entity(header);
}

/// Queues the entity the tick found on `from` moved to `to`, a cell
/// of the same superchunk.
#[inline]
pub fn move_to(turn: &mut Turn, from: CellIndex, to: CellIndex) {
    turn.move_entity(from, to);
}

/// Queues the entity the tick found on `at` removed.
#[inline]
pub fn remove(turn: &mut Turn, at: CellIndex) {
    turn.remove_entity(at);
}

/// Queues the entity the tick found on `left` made on `after`'s cell,
/// in another superchunk, of `after`'s type and to wake at its wake,
/// with `default` -- its type's default attribute, if it has one and
/// it is not among the others -- and `attributes`, the others.
#[inline]
pub fn cross(turn: &mut Turn, left: CellIndex, after: Header, default: &[AttributeBlock], attributes: &[AttributeBlock]) {
    turn.cross_entity(left, after, default, attributes);
}
