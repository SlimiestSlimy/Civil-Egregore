//! Civil Egregore's simulation: a world held whole
//! ([`SimulationHandle`]), its rules ticked over the hot bitplanes and the
//! entities, each read and written only through the handles its crate
//! gives, its halos moved, and every change between ticks its own call. What a rule makes of a turn is `../instructions`.
//!
//! The design: `docs/simulation.md`; function by function:
//! `docs/simulation_reference.md`.


pub mod halos;
pub mod simulation_api;
mod raw_compares_and_writes;
pub mod hot;
pub mod rules;
mod sampling;
pub mod simulation_handle;
pub mod simulation_time;
mod tick;
mod simulation_tick_state;

pub use sampling::{sample, sample_layer, sample_tiles_of_layer};
pub use halos::{HaloChange, Halos, COOL_TICKS, WARM_TICKS};
pub use hot::Hot;
pub use rules::{hot_entity_of, place_of, rules_of, worldgen_of, ChosenRules, Rule, RulePlace, TickCounts, MOST_RULES};
pub use simulation_handle::{ColdPoolPaging, SavedImage, SimulationHandle, WorldTick};
pub use tick::{threads_for, Simulation, TickReport};
pub use simulation_tick_state::conditional::{Compare, CountedWhenApplied, COUNTED_WHEN_APPLIED};
pub use simulation_tick_state::one_instruction::Write;
pub use simulation_tick_state::SimulationTickState;
