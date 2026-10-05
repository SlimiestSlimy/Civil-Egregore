//! Civil Egregore's instructions: what a rule is made of. Each is one small
//! thing on a superchunk's turn, a query of the simulation as much as
//! a change queued to it -- a step found, a cell sought, the
//! neighbours open to walk to -- written once here, light, and many: a rule of the cells or of an entity (`../mc_rules`,
//! `../entity_rules`) is a few of them put together, and holds only
//! what is its own.
//!
//! An instruction is built on what is public of the crates under it:
//! the simulation's turn -- cells read, writes and entity instructions
//! queued -- the terrain's layers (`../worldgen`) and the paths of
//! `../pathfinding`. None of those knows another; they meet here.
//!
//! Instructions are kept by what they do to the turn:
//!
//! | folder | instructions |
//! |---|---|
//! | `read/` | those that read, and queue nothing: cells, entities, the cells about a cell, the area, masks, walking |
//! | `write/` | those that queue a change: cells, entities, masks |
//!
//! One that reads and queues at once will be `rw/`'s; there is none
//! yet. Beside them, what they are asked in, no turn in it:
//!
//! | module | shape |
//! |---|---|
//! | `around` | the 3x3 cells about a cell as nine bits |
//! | `area` | the 16x16 cells about a cell, a row a word |
//! | `mask` | a square of cells as bits, 4 to 1,024 a side |
//!
//! A rule is written in these alone. The simulation under them reads
//! and writes cells and entities and no more: its [`Turn`] is what
//! every instruction is asked on, [`Simulation`] what ticks a rule,
//! [`TickReport`] what a tick says it did.
//!
//! Function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod area;
pub mod around;
pub mod mask;
pub mod read;
pub mod write;

pub use simulation::{Simulation, TickReport, Turn};
