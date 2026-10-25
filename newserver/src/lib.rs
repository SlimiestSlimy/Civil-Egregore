//! Civil Egregore's server, rewritten beside the old one (`server`) and
//! switched to once it does all the old one does: a world's cold side
//! ([`cold_storage`]), its hot superchunks in one store, and the
//! two-phase tick over them -- the read phase, then the compare and
//! write phase. Built a step at a time, each step's tests passing.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
// TEMPORARY, until the steps are wired together: most of the cold side
// has no caller yet.
#![allow(dead_code)]

mod cold_storage;
mod hot_superchunks;

// The crate's own tests, its internals theirs to reach: the tiers in
// `tests/`, and what they share -- the run's seed and the types they
// register.
#[cfg(test)]
#[path = "../tests/fine/main.rs"]
mod fine_tests;
#[cfg(test)]
#[allow(dead_code)] // a shared file: each test uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[cfg(test)]
#[path = "../tests/shared/registered_planes.rs"]
mod registered_planes;
