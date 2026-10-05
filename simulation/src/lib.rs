//! TileSim's simulation: the rules ticked over the hot bitplanes
//! (`../bitplane_manager`), which it reads and writes only through the
//! handles they give, and over the entities (`../entity_manager`),
//! likewise.
//!
//! | file | what is in it |
//! |---|---|
//! | `sampling` | Monte Carlo sampling: every set cell chosen with one probability, in Morton order, none wasted |
//! | `tick` | the tick: rules run superchunk by superchunk in two phases -- computing, writes queued for each superchunk they land in; applying, each superchunk its own |
//! | `turn/` | a superchunk's turn in the first phase: what a rule reads and queues, the cells about a cell, the entities, and the outbox |
//! | `../entity_manager/` | entities: a bucket a chunk, attributes added and removed at run time, a timer wheel a superchunk, instructions queued and applied in the tick |
//! | `transient_data` | the crate's `transient_data/`, out of git: what its runs leave behind |
//!
//! The design: `docs/simulation.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod around;
mod sampling;
mod tick;
mod turn;
pub mod transient_data;

pub use sampling::{sample, sample_layer};
pub use around::Around;
pub use tick::{threads_for, Simulation, TickReport};
pub use turn::{Area, Turn, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};
