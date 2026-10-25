//! The client's fine tier: each test pins one behaviour of one of its
//! modules alone, at the edges written by hand -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test -p client --lib fine_tests`

mod rendering;
