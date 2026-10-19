//! The fine tier of the workspace's own tests: each test pins one
//! higher-level behaviour, instant. A crate's own tests check its
//! invariants alone; what crosses crates is tested here
//! (`docs/testing_protocol.md`).
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;

mod basegame_api;
mod docs;
mod type_registry;
