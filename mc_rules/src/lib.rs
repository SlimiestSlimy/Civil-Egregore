//! Civil Egregore's rules of the cells, a file each: a tick of Monte
//! Carlo sampling on a superchunk's turn, and the writes it queues,
//! through `instructions` alone.
//!
//! What the rules are: `docs/mc_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod grass;
pub mod trees;
