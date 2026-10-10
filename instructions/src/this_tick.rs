//! The tick a rule is run in, and its superchunk's lot: what a rule
//! asks of its turn that is neither a cell nor an entity
//! (`docs/instructions.md`, "Rules ask instructions, and nothing else").

use coordinates::SuperchunkIndex;
use simulation::Turn;
use utilities::rng::Rng;

/// The tick running.
#[inline]
pub fn now(turn: &Turn) -> u64 {
    turn.now()
}

/// The random numbers of the turn's superchunk: its own, going on from
/// the last tick's, so what a rule draws is the same on any number of
/// threads.
#[inline]
pub fn random<'turn>(turn: &'turn mut Turn) -> &'turn mut Rng {
    turn.random()
}

/// The superchunk whose turn it is.
#[inline]
pub fn superchunk(turn: &Turn) -> SuperchunkIndex {
    turn.superchunk()
}
