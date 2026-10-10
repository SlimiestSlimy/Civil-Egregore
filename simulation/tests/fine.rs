//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
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
