//! The fine tier of the workspace's own tests: each test pins one
//! higher-level behaviour, instant. A crate's own tests check its
//! invariants alone; what crosses crates is tested here
//! (`docs/testing_protocol.md`).
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
// Hot arenas of stone and walkers on them: actions' tests' own, shared.
#[path = "../../server/simulation/tests/shared/arenas.rs"]
mod arenas;
#[path = "../shared/walkers.rs"]
mod walkers;

mod acts_over_a_border;
mod compare_and_write;
mod docs;
mod entities_moving;
mod entities_waking;
mod entity_over_a_border;
mod instructions;
