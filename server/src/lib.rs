//! Civil Egregore's server: the world as a whole, held by the one
//! crate that puts the rest together -- started, made from a seed or
//! opened from a folder as it was saved, the one way ([`start_world`]),
//! ticked, and saved ([`save`]). A
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
pub mod internals_for_root_tests;
pub mod params;
mod pathfinding;
pub mod simulation;
mod save_and_load;
mod world_access;
mod world_hash;
mod world_start;

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

pub use crate::simulation::hot::halos_about;
pub use crate::simulation::halos::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use crate::simulation::rules::{ChosenRules, RulePlace, TickCounts};
pub use crate::simulation::simulation_handle::WorldTick;
pub use crate::chunk_storage::disk::DiskError;
pub use save_and_load::{read_types, save, worlds_in, write_types, SaveWritten};
pub use world_hash::{world_hash, WorldHash};
pub use world_start::{drawn_seed, seed_fit_to_start};

use crate::chunk_storage::disk::WorldInfo;
use utilities::coordinates::WORLD_MIDDLE;
use crate::simulation::simulation_api::WorldgenCode;
use crate::simulation::{ColdPoolPaging, SimulationHandle};
use std::sync::Arc;
use protocol::type_registry::FrozenTypeRegistry;
use protocol::server_parameters::{CleanServerParameters, COLD_POOL_BYTES, WORLD};
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

/// A world: what its file says it is -- its generation's parameters among
/// it -- what generates it, the types it runs with, and the world
/// itself as the simulation holds it -- which does all there is to do
/// to it; the server only coordinates.
pub struct World {
    /// What the world is, as its file keeps it: the simulation's own
    /// parts -- its tick, those unpopulated -- read from the simulation
    /// when it is saved.
    pub(crate) world_info: WorldInfo,
    /// What generates it: the library of its type hooked into
    /// generating the world, asked of what the server is asked of it.
    pub(crate) worldgen: WorldgenCode,
    /// The types it runs with: frozen as it starts, the same as long as
    /// it runs.
    pub(crate) frozen_type_registry: Arc<FrozenTypeRegistry>,
    /// The world itself, as the simulation holds it.
    pub(crate) simulation_handle: SimulationHandle,
}

impl World {
    /// A world with nothing in it, at `world_info`'s tick: what starting
    /// and loading build on. Of `frozen_type_registry`, whose hooks give the
    /// simulation its rules, what generates it and what it is hot about --
    /// or why it cannot run. On `thread_count` threads, every one the
    /// machine has if none is given.
    fn empty_world(world_info: WorldInfo, thread_count: Option<usize>, frozen_type_registry: Arc<FrozenTypeRegistry>) -> Result<Self, String> {
        let worldgen = crate::simulation::worldgen_of(&frozen_type_registry)?;
        // Its numbers as its generation reads them: every one it knows, none it does not.
        let world_info = WorldInfo { worldgen_parameters: (worldgen.parameters_read)(&world_info.worldgen_parameters), ..world_info };
        let simulation_handle = SimulationHandle::new(&world_info, &frozen_type_registry, thread_count, ColdPoolPaging { folder: paging_folder(), bytes_kept: COLD_POOL_BYTES_KEPT })?;
        // Those unpopulated are the simulation's to keep from here on.
        Ok(Self { world_info: WorldInfo { unpopulated: Vec::new(), ..world_info }, worldgen, frozen_type_registry, simulation_handle })
    }

    /// The types it runs with.
    pub fn frozen_type_registry(&self) -> &FrozenTypeRegistry {
        &self.frozen_type_registry
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

/// Why a world was not started.
#[derive(Debug)]
pub enum WorldNotStarted {
    /// The world asked to be opened is not a save, or not one this
    /// server reads: which file, and why.
    Unreadable(DiskError),
    /// The parameters asked what cannot be -- a number out of range,
    /// forced hot with no side -- or the types cannot run a world.
    Refused(String),
    /// The world made new could not be saved in the folder it was given.
    Unsaved(DiskError),
}

impl std::fmt::Display for WorldNotStarted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Unreadable(disk_error) => write!(formatter, "{disk_error}"),
            Self::Refused(why_not) => write!(formatter, "{why_not}"),
            Self::Unsaved(disk_error) => write!(formatter, "made, and not saved: {disk_error}"),
        }
    }
}

/// The one way a world is started, for play and for tests alike: as
/// `parameters` say, of `frozen_type_registry` -- registered and frozen
/// before anything of it, by whoever starts it (`protocol::world_types`;
/// which, a save's or a new world's: [`holds_a_world`]) -- on the
/// threads they say. Given a folder ([`WORLD`]) a world is saved in, that
/// world opened as it was saved, the registry checked against its types
/// file. Else made new, nothing in it yet, the superchunks it starts
/// with hot -- every one of a world with a size forced hot, else its
/// middle -- each populated by its generation the first tick, as every
/// superchunk generated is (`docs/server.md`, "Made from a seed"); and,
/// given a folder, saved there as it starts. Or why not.
pub fn start_world(parameters: &CleanServerParameters, frozen_type_registry: &Arc<FrozenTypeRegistry>) -> Result<World, WorldNotStarted> {
    let thread_count = world_start::thread_count_of(parameters).map_err(WorldNotStarted::Refused)?;
    let folder = parameters.text(WORLD).map(utilities::settings::world);
    if let Some(world_folder) = folder.as_deref().filter(|world_folder| holds_a_world(world_folder)) {
        let bytes_kept = parameters.integer(COLD_POOL_BYTES).map_or(Ok(COLD_POOL_BYTES_KEPT), u64::try_from).map_err(|_| WorldNotStarted::Refused(format!("--{COLD_POOL_BYTES}: a count of bytes")))?;
        return save_and_load::open_world(world_folder, bytes_kept, thread_count, frozen_type_registry).map_err(WorldNotStarted::Unreadable);
    }
    let worldgen = crate::simulation::worldgen_of(frozen_type_registry).map_err(WorldNotStarted::Refused)?;
    let world_info = world_start::new_world_info(frozen_type_registry, &worldgen, parameters).map_err(WorldNotStarted::Refused)?;
    let mut world = World::empty_world(world_info, thread_count, Arc::clone(frozen_type_registry)).map_err(WorldNotStarted::Refused)?;
    let every_superchunk = world.simulation_handle.halos().hot.all_superchunks();
    let start_superchunks = if every_superchunk.is_empty() { vec![WORLD_MIDDLE] } else { every_superchunk };
    world.simulation_handle.keep_hot(&start_superchunks);
    world.simulation_handle.mark_unpopulated(&start_superchunks);
    if let Some(world_folder) = &folder {
        save(world_folder, &mut world).map_err(WorldNotStarted::Unsaved)?;
    }
    Ok(world)
}

/// Whether a world is saved in `folder` -- what a world started of it
/// opens, its type registry frozen as its save has it -- or none is, and
/// a world started of it is made new there.
pub fn holds_a_world(folder: &std::path::Path) -> bool {
    folder.join(chunk_storage::disk::WORLD_FILE).exists()
}
