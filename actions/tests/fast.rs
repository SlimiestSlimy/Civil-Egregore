//! The fast tier: instructions asked on a small hot arena -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/arenas.rs"]
mod arenas;

#[path = "fast/around.rs"]
mod around;

#[path = "fast/area.rs"]
mod area;

#[path = "fast/mask.rs"]
mod mask;
