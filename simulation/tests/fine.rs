//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

#[path = "fine/entities_waking.rs"]
mod entities_waking;

#[path = "fine/entities_moving.rs"]
mod entities_moving;

#[path = "fine/instructions.rs"]
mod instructions;
