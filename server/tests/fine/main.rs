//! The server's fine tier: each test pins one behaviour of one of its
//! modules alone, at the edges written by hand and at cases drawn from
//! the run's seed -- instant. What needs the base game's types is
//! tested at the root (`tests/fine`).
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../../utilities/test_seed.rs"]
mod test_seed;
#[path = "../shared/arenas.rs"]
mod arenas;
#[path = "../shared/areas.rs"]
mod areas;
#[path = "../shared/chunks.rs"]
mod chunks;

mod bitplane_manager;
mod pathfinding;
mod simulation;
