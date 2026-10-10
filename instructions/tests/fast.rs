//! The fast tier: instructions asked on a small hot arena -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod tests;

#[path = "fast/walking.rs"]
mod walking;

#[path = "fast/area.rs"]
mod area;

#[path = "fast/mask.rs"]
mod mask;

#[path = "fast/shifted.rs"]
mod shifted;
