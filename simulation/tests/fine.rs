//! The fine tier: each test pins one behaviour of the simulation alone, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;

#[path = "fine/simulation_time.rs"]
mod simulation_time;
