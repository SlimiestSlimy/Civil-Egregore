//! TileSim's instructions: what a rule is made of. Each is one small
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
//! | module | instructions |
//! |---|---|
//! | `cells` | a layer at a cell asked and queued, a wide plane's number, the square about a cell, the going over the cells sampled |
//! | `entities` | the going over the entities waking; one made, put to sleep, committed as changed, removed |
//! | `around` | the 3x3 cells about a cell as nine bits: read, those entities stand on, one free, one picked |
//! | `area` | the 16x16 cells about a cell, a row a word: read, those entities stand on, and the tiles further off |
//! | `mask` | a square of cells as bits, 4 to 1,024 a side: a layer read into one, whole or under another, and set or cleared under one |
//! | `walking` | the steps the terrain's walls leave open, the step towards a cell or the nearest of some, and towards the nearest of a layer's however far off in reach |
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
pub mod cells;
pub mod entities;
pub mod mask;
pub mod walking;

pub use simulation::{Simulation, TickReport, Turn};
