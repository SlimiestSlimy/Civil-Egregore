//! The server's complete tier: what takes too long for the fast one,
//! `#[ignore]`d -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release -p server --lib complete_tests -- --ignored`

// What the tiers share is declared at the crate's root, beside them.

mod flock_over_borders;
