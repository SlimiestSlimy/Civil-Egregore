//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

#[path = "fine/paths_and_waves.rs"]
mod paths_and_waves;

#[path = "fine/walls.rs"]
mod walls;
