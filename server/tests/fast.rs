//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod tests;

#[path = "fast/commands.rs"]
mod commands;

#[path = "fast/halos.rs"]
mod halos;

#[path = "fast/world.rs"]
mod world;

#[path = "fast/host.rs"]
mod host;

#[path = "fast/grass.rs"]
mod grass;

#[path = "fast/sheep.rs"]
mod sheep;

#[path = "fast/world_hash.rs"]
mod world_hash;
