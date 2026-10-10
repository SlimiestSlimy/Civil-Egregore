//! Civil Egregore's simulation: the rules ticked over the hot bitplanes
//! and the entities, each read and written only through the handles
//! its crate gives. What a rule makes of a turn is `../instructions`.
//!
//! The design: `docs/simulation.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod halos;
pub mod hot;
mod sampling;
mod tick;
mod turn;
pub mod transient_data;

pub use sampling::{sample, sample_layer};
pub use halos::{HaloChange, Halos, Held, COOL_TICKS, WARM_TICKS};
pub use hot::Hot;
pub use tick::{threads_for, Simulation, TickReport};
pub use turn::Turn;
