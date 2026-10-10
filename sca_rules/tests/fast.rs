//! The fast tier: each rule of the cells alone on a plain the server makes, run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

// What every test of a rule on a world shares: the server's.
#[path = "../../server/tests/tests.rs"]
mod tests;

#[path = "fast/grass.rs"]
mod grass;

#[path = "fast/trees.rs"]
mod trees;
