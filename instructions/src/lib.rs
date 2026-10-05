//! TileSim's instructions: what a rule is made of. Each is one small
//! thing asked on a superchunk's turn -- a step found, a cell sought,
//! the neighbours open to walk to -- written once here, light, and
//! many: a rule of the cells or of an entity (`../mc_rules`,
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
//! | `walking` | the steps the terrain's walls leave open, the step towards a cell or the nearest of some, and towards the nearest of a layer's however far off in reach |
//!
//! Function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod walking;
