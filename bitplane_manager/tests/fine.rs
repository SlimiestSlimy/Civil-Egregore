//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

#[path = "fine/hot_bitmaps.rs"]
mod hot_bitmaps;

#[path = "fine/writing_back.rs"]
mod writing_back;

#[path = "fine/counts_and_windows.rs"]
mod counts_and_windows;

#[path = "fine/writes.rs"]
mod writes;
