//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/chunks.rs"]
mod chunks;

#[path = "fine/hot_bitmaps.rs"]
mod hot_bitmaps;

#[path = "fine/writing_back.rs"]
mod writing_back;

#[path = "fine/counts_and_windows.rs"]
mod counts_and_windows;

#[path = "fine/writes.rs"]
mod writes;
