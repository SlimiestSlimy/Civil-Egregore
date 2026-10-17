//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/areas.rs"]
mod areas;

#[path = "fine/paths_and_waves.rs"]
mod paths_and_waves;

#[path = "fine/walls.rs"]
mod walls;

#[path = "fine/morton.rs"]
mod morton;
