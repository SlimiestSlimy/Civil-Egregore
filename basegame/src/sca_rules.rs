//! Civil Egregore's rules of the cells, a stochastic cell automaton
//! (SCA), a file each: a tick of its sampling on a superchunk's turn, and the writes it queues,
//! through `actions` alone.
//!
//! What the rules are: `docs/sca_rules.md`; function by function:
//! `docs/reference.md`.


pub mod grass;
pub mod trees;
