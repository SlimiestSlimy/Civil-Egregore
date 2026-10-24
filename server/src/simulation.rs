//! Civil Egregore's simulation: a world held whole
//! ([`SimulationHandle`]), its rules ticked over the hot bitplanes and the
//! entities, each read and written only through the handles its crate
//! gives, its halos moved, and every change between ticks its own call. What a rule makes of a turn is `../instructions`.
//!
//! The design: `docs/simulation.md`; function by function:
//! `docs/simulation_reference.md`.


pub(crate) mod halos;
pub mod simulation_api;
mod raw_compares_and_writes;
pub(crate) mod hot;
pub(crate) mod rules;
mod sampling;
pub(crate) mod simulation_handle;
pub(crate) mod tick;
mod simulation_tick_state;

pub(crate) use halos::HaloChange;
pub(crate) use rules::{worldgen_of, ChosenRules, RulePlace, TickCounts};
pub(crate) use simulation_handle::{ColdPoolPaging, SavedImage, SimulationHandle, WorldTick};
pub(crate) use tick::{Simulation, TickReport};
pub(crate) use simulation_tick_state::conditional::Compare;
pub(crate) use simulation_tick_state::one_instruction::Write;
pub(crate) use simulation_tick_state::SimulationTickState;
// Names only the crate's tests use.
#[cfg(test)]
pub(crate) use {sampling::sample, tick::threads_for};
