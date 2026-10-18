//! The tick a rule is run in, and its superchunk's lot: what a rule
//! asks of its turn that is neither a cell nor an entity
//! (`docs/actions.md`, "Rules ask actions, and nothing else").

use utilities::coordinates::SuperchunkIndex;
use crate::simulation::SimulationTickState;
use utilities::rng::RandomSource;

/// The tick running.
#[inline]
pub fn current_tick(tick_state: &SimulationTickState) -> u64 {
    tick_state.current_tick()
}

/// The random numbers of the turn's superchunk: its own, going on from
/// the last tick's, so what a rule draws is the same on any number of
/// threads.
#[inline]
pub fn random_stream<'tick_state>(tick_state: &'tick_state mut SimulationTickState) -> &'tick_state mut RandomSource {
    tick_state.random_stream()
}

/// The superchunk whose turn it is.
#[inline]
pub fn superchunk(tick_state: &SimulationTickState) -> SuperchunkIndex {
    tick_state.superchunk()
}
