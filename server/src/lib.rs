//! Civil Egregore's server: the world as a whole, held by the one
//! crate that puts the rest together -- made in three steps on its
//! folder, created, set up and run ([`world_creation`]), ticked, and
//! saved ([`save`]). A
//! renderer, or the program, is a client of it.
//!
//! The design: `docs/server.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

// The server's own parts, private: the simulation shows its API alone.
mod bitplane_manager;
mod chunk_storage;
mod client_connection;
mod entity_manager;
pub mod host;
pub mod params;
mod pathfinding;
pub mod simulation;
mod save_and_load;
mod world_access;
pub mod world_creation;
mod world_hash;

// The crate's own tests, its internals theirs to reach: the tiers in
// `tests/`, and what they share -- the run's seed, and the chunks,
// arenas, walkers and areas they are drawn on.
#[cfg(test)]
#[path = "../tests/fine/main.rs"]
mod fine_tests;
#[cfg(test)]
#[path = "../tests/fast/main.rs"]
mod fast_tests;
#[cfg(test)]
#[path = "../tests/complete/main.rs"]
mod complete_tests;
#[cfg(test)]
#[allow(dead_code)] // a shared file: each test uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[cfg(test)]
#[path = "../tests/shared/arenas.rs"]
mod arenas;
#[cfg(test)]
#[path = "../tests/shared/areas.rs"]
mod areas;
#[cfg(test)]
#[path = "../tests/shared/chunks.rs"]
mod chunks;
#[cfg(test)]
#[path = "../tests/shared/walkers.rs"]
mod walkers;
#[cfg(test)]
#[path = "../tests/shared/test_types.rs"]
mod test_types;
#[cfg(test)]
#[path = "../tests/shared/collision.rs"]
mod collision;
#[cfg(test)]
#[path = "../tests/shared/test_game.rs"]
mod test_game;
#[cfg(test)]
#[path = "../tests/shared/whole_worlds.rs"]
mod whole_worlds;

pub use crate::simulation::hot::halos_about;
pub use crate::simulation::halos::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use crate::simulation::rules::{ChosenRules, RulePlace, TickCounts};
pub use crate::simulation::simulation_handle::WorldTick;
pub use crate::chunk_storage::disk::DiskError;
pub use save_and_load::{read_types, save, worlds_in, write_types, SaveWritten};
pub use world_hash::{world_hash, WorldHash};
pub use world_creation::{create_world, make_world, make_world_of, run_world, set_up_world, WorldNotMade, MOD_CONFIG, WORLD_FOLDERS};

use crate::chunk_storage::disk::WorldInfo;
use crate::simulation::simulation_api::SetUpWorldgen;
use crate::simulation::{ColdPoolPaging, SimulationHandle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use protocol::kinds::server::client_call::WorldClientCalls;
use protocol::type_registry::FrozenTypeRegistry;
use protocol::server_parameters::CleanServerParameters;
use protocol::world_types::RegisterTypes;

/// Bytes of cold superchunks' images a world keeps in memory, unless
/// told another number ([`World::keep_cold_pool_within`]): past it
/// they are paged to disk (`docs/server.md`, "The cold pool paged").
pub const COLD_POOL_BYTES_KEPT: u64 = 2 << 30;

/// Where running worlds page their cold pools' images, a folder each,
/// gone with its world (`chunk_storage`'s docs, "Paged to disk"): in
/// Civil Egregore's folder on the machine, never the workspace's.
pub fn paging_folder() -> std::path::PathBuf {
    utilities::settings::folder().join("paging")
}

/// A world: what its file says it is, its folder, the types it runs
/// with, and the world itself as the simulation holds it -- which does
/// all there is to do to it; the server only coordinates.
pub struct World {
    /// What the world is, as its file keeps it: the simulation's own
    /// part -- its tick -- read from the simulation when it is saved.
    pub(crate) world_info: WorldInfo,
    /// Its folder: where it was made, or last saved.
    pub(crate) folder: PathBuf,
    /// The types it runs with: frozen as it starts, the same as long as
    /// it runs.
    pub(crate) frozen_type_registry: Arc<FrozenTypeRegistry>,
    /// The world itself, as the simulation holds it.
    pub(crate) simulation_handle: SimulationHandle,
    /// The functions of its types a client may call, set up with its
    /// folder's mod config: what a call is answered by.
    pub(crate) client_calls: WorldClientCalls,
}

impl World {
    /// The world in `folder` with nothing in it, at `world_info`'s tick and
    /// of its seed, generated as `worldgen` -- set up from its config -- says:
    /// what running it builds on. Of `frozen_type_registry`, whose hooks give the
    /// simulation its rules and what it is hot about -- or why it cannot
    /// run. On `thread_count` threads, every one the machine has if none
    /// is given.
    fn empty_world(world_info: WorldInfo, folder: &Path, worldgen: SetUpWorldgen, thread_count: Option<usize>, frozen_type_registry: Arc<FrozenTypeRegistry>) -> Result<Self, String> {
        let simulation_handle = SimulationHandle::new(&world_info, worldgen, &frozen_type_registry, thread_count, ColdPoolPaging { folder: paging_folder(), bytes_kept: COLD_POOL_BYTES_KEPT })?;
        let client_calls = WorldClientCalls::set_up(&frozen_type_registry, &folder.join(MOD_CONFIG))?;
        Ok(Self { world_info, folder: folder.to_path_buf(), frozen_type_registry, simulation_handle, client_calls })
    }

    /// The types it runs with.
    pub fn frozen_type_registry(&self) -> &FrozenTypeRegistry {
        &self.frozen_type_registry
    }

    /// Its seed.
    pub fn seed(&self) -> u64 {
        self.simulation_handle.seed()
    }

    /// Its folder: where it was made, or last saved.
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

/// Runs what `clean_parameters` ask for: the server's one way in from
/// a command line -- with `register_types`, what registers the game's
/// types: each world's are registered and frozen before it is made or
/// opened (`protocol::world_types`). A world run on a host, its clients
/// connecting one at a time ([`client_connection`]) -- closing once none
/// is, if told to ([`params::CLOSE_IF_NO_CLIENTS`]).
pub fn start(clean_parameters: &CleanServerParameters, register_types: RegisterTypes) -> Result<String, String> {
    client_connection::serve_clients(clean_parameters, register_types)
}

