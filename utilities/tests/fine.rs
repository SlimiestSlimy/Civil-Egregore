//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[path = "fine/fixed_list.rs"]
mod fixed_list;

#[path = "fine/hash.rs"]
mod hash;

#[path = "fine/process_memory.rs"]
mod process_memory;

#[path = "fine/rng.rs"]
mod rng;

#[path = "fine/table.rs"]
mod table;

#[path = "fine/commands.rs"]
mod commands;

#[path = "fine/dispatcher.rs"]
mod dispatcher;

#[path = "fine/settings.rs"]
mod settings;

#[path = "fine/tuning.rs"]
mod tuning;

#[path = "fine/chance.rs"]
mod chance;
