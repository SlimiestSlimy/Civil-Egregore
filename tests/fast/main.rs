//! The fast tier of the workspace's own tests: small worlds and corpora
//! run and judged, seconds (`docs/testing_protocol.md`).
//!
//! `cargo test --release --test fast`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "../shared/worlds.rs"]
mod worlds;
// Hot arenas of stone and walkers on them: actions' tests' own, shared.
#[path = "../../server/simulation/tests/shared/arenas.rs"]
mod arenas;

mod collision;
mod commands;
mod double_writes;
mod flock_over_borders;
mod grass;
mod halos;
mod host;
mod paging;
mod pipe;
mod sheep;
mod tick;
mod trees;
mod walking;
mod weather;
mod world;
mod world_hash;
