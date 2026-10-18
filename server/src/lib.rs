//! Civil Egregore's server: the world as a whole, held by the one
//! crate that puts the rest together -- made from a seed ([`start_world`]),
//! ticked, saved ([`save`]) and loaded ([`load_world`]) as it was. A
//! renderer, or the program, is a client of it.
//!
//! The design: `docs/server.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod bitplane_manager;
pub mod chunk_storage;
pub mod commands;
pub mod entity_manager;
pub mod host;
pub mod params;
pub mod pathfinding;
pub mod simulation;
mod pipe_end;
mod save_and_load;
mod world_access;
mod world_hash;
mod world_start;

pub use crate::simulation::hot::halos_about;
pub use crate::simulation::{ChosenRules, HaloChange, RulePlace, TickCounts, WorldTick, COOL_TICKS, WARM_TICKS};
pub use save_and_load::{load_keeping, load_world, read_types, save, worlds_in, write_types, SaveWritten};
pub use world_hash::{world_hash, WorldHash};
pub use world_start::{drawn_seed, seed_fit_to_start, WorldSize, WorldStart};

use crate::chunk_storage::disk::WorldInfo;
use utilities::coordinates::WORLD_MIDDLE;
use crate::simulation::simulation_api::WorldgenCode;
use crate::simulation::{ColdPoolPaging, SimulationHandle};
use std::sync::Arc;
use protocol::type_registry::FrozenTypes;
use protocol::world_types::GameTypes;

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

/// A world: what its file says it is -- its generation's numbers among
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
    pub(crate) frozen_types: Arc<FrozenTypes>,
    /// The world itself, as the simulation holds it.
    pub(crate) simulation_handle: SimulationHandle,
}

impl World {
    /// A world with nothing in it, at `world_info`'s tick: what starting
    /// and loading build on. Of `frozen_types`, whose hooks give the
    /// simulation its rules, what generates it and what it is hot about --
    /// or why it cannot run. On `thread_count` threads, every one the
    /// machine has if none is given.
    fn empty_world(world_info: WorldInfo, thread_count: Option<usize>, frozen_types: Arc<FrozenTypes>) -> Result<Self, String> {
        let worldgen = crate::simulation::worldgen_of(&frozen_types)?;
        // Its numbers as its generation reads them: every one it knows, none it does not.
        let world_info = WorldInfo { generation_numbers: (worldgen.numbers_read)(&world_info.generation_numbers), ..world_info };
        let simulation_handle = SimulationHandle::new(&world_info, &frozen_types, thread_count, ColdPoolPaging { folder: paging_folder(), bytes_kept: COLD_POOL_BYTES_KEPT })?;
        // Those unpopulated are the simulation's to keep from here on.
        Ok(Self { world_info: WorldInfo { unpopulated: Vec::new(), ..world_info }, worldgen, frozen_types, simulation_handle })
    }

    /// The types it runs with.
    pub fn frozen_types(&self) -> &FrozenTypes {
        &self.frozen_types
    }
}

/// Runs what `clean_parameters` ask for: the server's one way in from
/// a command line, whichever command is nested in them -- with
/// `game_types`, what registers the game's types: each world's are
/// registered and frozen before it is made or loaded
/// (`protocol::world_types`).
pub fn start(clean_parameters: &protocol::server_parameters::CleanServerParameters, game_types: GameTypes) -> Result<String, String> {
    if let Some(new_parameters) = clean_parameters.nested(params::NEW) {
        return commands::new(utilities::seed::given_or_fresh_seed(clean_parameters.hex(params::SEED)), new_parameters, game_types);
    }
    if let Some(run_parameters) = clean_parameters.nested(params::RUN) {
        return commands::run_saved_world(run_parameters, game_types);
    }
    if let Some(info_parameters) = clean_parameters.nested(params::INFO) {
        return commands::describe_saved_world(info_parameters);
    }
    if clean_parameters.nested(params::PIPE).is_some() {
        return pipe_end::serve_the_pipe(game_types);
    }
    Err("no command given: --help says them".to_string())
}

/// A world as `options` say, nothing in it yet: the superchunks it starts
/// with hot -- every one of a world with a size forced hot, else its
/// middle -- each populated by its generation the first tick, as every
/// superchunk generated is (`docs/server.md`, "Made from a seed"). Of
/// `frozen_types`: a world loaded is of its own.
pub fn start_world(options: WorldStart, frozen_types: &Arc<FrozenTypes>) -> World {
    let (side_in_superchunks, forced_hot) = match options.world_size {
        WorldSize::NoEnd => (None, false),
        WorldSize::SquareOfSide { side_superchunks, forced_hot } => (Some(side_superchunks), forced_hot),
    };
    let world_info = WorldInfo { seed: options.seed, tick: 0, layers: frozen_types.layer_types(), side_in_superchunks, forced_hot, camera_loads: options.camera_loads && !forced_hot, unpopulated: Vec::new(), generation_numbers: options.generation_numbers };
    let mut world = World::empty_world(world_info, options.thread_count, Arc::clone(frozen_types)).expect("the types handed the server run");
    let every_superchunk = world.simulation_handle.halos().hot.all_superchunks();
    let start_superchunks = if every_superchunk.is_empty() { vec![WORLD_MIDDLE] } else { every_superchunk };
    world.simulation_handle.keep_hot(&start_superchunks);
    world.simulation_handle.mark_unpopulated(&start_superchunks);
    world
}
