//! Civil Egregore's rules of the cells: every rule the cells run by, a file
//! each -- a tick of Monte Carlo sampling on a superchunk's turn, and
//! the writes it queues. Entities are a crate of their own
//! (`entity_rules/`); the world ticks both together (`server/`).
//!
//! | module | rule |
//! |---|---|
//! | `grass` | grass spreading over dirt, and decaying |
//! | `trees` | trees growing by stages, spreading and dying |
//!
//! A rule sees the simulation through `instructions` alone, so it holds
//! no world: the worlds its rules are tested and measured on are the
//! server's (`server/tests/fast/grass.rs`, `server/src/diagnostics/`).
//!
//! What the rules are: `docs/mc_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod grass;
pub mod trees;
