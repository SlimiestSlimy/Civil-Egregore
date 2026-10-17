//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../test_seed.rs"]
mod test_seed;
#[path = "fine/fixed_list.rs"]
mod fixed_list;

#[path = "fine/hash.rs"]
mod hash;

#[path = "fine/rng.rs"]
mod rng;

#[path = "fine/dispatcher.rs"]
mod dispatcher;

#[path = "fine/settings.rs"]
mod settings;

#[path = "fine/tuning.rs"]
mod tuning;

#[path = "fine/chance.rs"]
mod chance;

#[path = "fine/wire.rs"]
mod wire;

#[path = "fine/bitmap.rs"]
mod bitmap;

#[path = "fine/coordinates.rs"]
mod coordinates;
