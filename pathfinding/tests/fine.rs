//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

#[path = "fine/paths_and_waves.rs"]
mod paths_and_waves;

#[path = "fine/walls.rs"]
mod walls;
