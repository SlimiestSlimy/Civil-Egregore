//! TileSim's simulation: the rules ticked over the hot bitplanes
//! (`../bitplane_manager`), which it reads and writes only through the
//! handles they give, and over the entities (`../entity_manager`),
//! likewise.
//!
//! | file | what is in it |
//! |---|---|
//! | `sampling` | Monte Carlo sampling: every set cell chosen with one probability, in Morton order, none wasted |
//! | `hot` | which superchunks are to be hot: the world's size, and the hot entity, whose halos are |
//! | `halos` | the halos moved: superchunks warming and cooling, each due at a tick, made hot and cold by jobs off the tick |
//! | `tick` | the tick: rules run superchunk by superchunk in two phases -- computing, writes queued for each superchunk they land in; applying, each superchunk its own |
//! | `turn/` | a superchunk's turn in the first phase: the cells and entities a rule reads, the writes and instructions it queues, and the outbox |
//! | `../entity_manager/` | entities: a bucket a chunk, attributes added and removed at run time, a timer wheel a superchunk, instructions queued and applied in the tick |
//! | `transient_data` | the crate's `transient_data/`, out of git: what its runs leave behind |
//!
//! It reads and writes cells and entities, and ticks: what a rule makes
//! of that is `../instructions`, which the rules are written in.
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
