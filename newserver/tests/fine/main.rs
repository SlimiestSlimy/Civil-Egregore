//! newserver's fine tier: each test pins one behaviour of one of its
//! modules alone, at the edges written by hand and at cases drawn from
//! the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test -p newserver --lib fine_tests`

// What the tiers share is declared at the crate's root, beside them.

mod cold_storage;
mod hot_plane_reads;
mod hot_planes;
