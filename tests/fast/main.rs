//! The fast tier of the workspace's own tests: small worlds and corpora
//! run and judged, seconds (`docs/testing_protocol.md`).
//!
//! `cargo test --release --test fast`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "../shared/worlds.rs"]
mod worlds;
// The tests' rule, setting up and reading a world in its ticks.
#[path = "../shared/probe.rs"]
mod probe;
// Trees planted on a bare plain, what both tiers' trees tests start from.
#[path = "../shared/tree_planting.rs"]
mod tree_planting;

mod collision;
mod commands;
mod double_writes;
mod grass;
mod host;
mod client_connection;
mod sheep;
mod trees;
mod walls;
mod world_hash;
