//! General-purpose utilities, shared by every crate in TileSim and
//! owned by none.
//!
//! | module | what it is |
//! |---|---|
//! | [`diagnostics`] | what every crate's diagnostics are made with: the table printer and a measurement's report, and the process's memory |
//! | [`transient_data`] | where a crate's runs leave what they make, out of git |
//! | [`rng`] | a seeded random source, whose whole state is one word |
//! | [`hash`] | a key's slot in a table, by Fibonacci hashing, and a word's bits mixed, SplitMix64's way |
//! | [`fixed_list`] | a list of fixed capacity, allocated once, that never grows |
//! | [`commands`] | a command line's first word found among a crate's commands and handed the rest; each command's parameters declared once |
//! | [`seed`] | the one seed every crate's tests and tools start from, rolled every few runs |
//! | [`dispatcher`] | the threads, started once and kept: a job run on all at once, a part each, and jobs queued for whichever is free |
//! | [`cache`] | memory asked of the processor's caches ahead of its being read |
//!
//! The design: `docs/utilities.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod cache;
pub mod commands;
pub mod diagnostics;
pub mod dispatcher;
pub mod fixed_list;
pub mod hash;
pub mod rng;
pub mod seed;
pub mod transient_data;
