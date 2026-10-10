//! Groups: several things a rule queues -- compare-and-writes on
//! cells, what an entity comes to, counts -- applied all or none
//! (`docs/instructions.md`, "Compare-and-write and groups").

use simulation::Turn;

/// Starts a group: every cell written and every entity instruction
/// queued until [`end`] is applied only if each cell written still
/// holds what the rule saw -- all of them, or none. A group lands in
/// one superchunk.
#[inline]
pub fn start(turn: &mut Turn) {
    turn.group_start();
}

/// Adds one to the rule's count `counted` if the group being queued
/// is applied.
#[inline]
pub fn count(turn: &mut Turn, counted: usize) {
    turn.count_if_applied(counted as u32);
}

/// Ends the group started.
#[inline]
pub fn end(turn: &mut Turn) {
    turn.group_end();
}
