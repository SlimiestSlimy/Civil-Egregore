//! The server's fine tier: each test pins one behaviour of one of its
//! modules alone, at the edges written by hand and at cases drawn from
//! the run's seed -- instant. What needs the base game's types is
//! tested at the root (`tests/fine`).
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test -p server --lib fine_tests`

// What the tiers share is declared at the crate's root, beside them.

mod bitplane_manager;
mod chunk_storage;
mod pathfinding;
mod simulation;
