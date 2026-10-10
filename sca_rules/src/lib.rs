//! Civil Egregore's rules of the cells, a stochastic cell automaton
//! (SCA), a file each: a tick of its sampling on a superchunk's turn, and the writes it queues,
//! through `instructions` alone.
//!
//! What the rules are: `docs/sca_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod grass;
pub mod trees;
