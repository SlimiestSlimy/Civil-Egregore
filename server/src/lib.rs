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

pub mod commands;
pub mod halos;
pub mod host;
pub mod params;
pub mod rules;
mod save_and_load;
mod tick;
mod world_access;
mod world_hash;
mod world_start;

pub use halos::HOT_ENTITY;
pub use simulation::hot::halos_about;
pub use simulation::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use rules::{place_of, rules_of, ChosenRules, Rule, RulePlace, TickCounts, MOST_RULES};
pub use save_and_load::{load_keeping, load_world, read_types, save, worlds_in, write_types, SaveWritten, TYPES_FILE};
pub use tick::WorldTick;
pub use world_hash::{world_hash, WorldHash};
pub use world_start::{drawn_seed, seed_with_land, WorldSize, WorldStart, FLOCK};

use bitplane_manager::{BitmapArena, Write, WriteOp};
use chunk_storage::disk::WorldInfo;
use chunk_storage::ChunkStorage;
pub use worldgen::WorldGeneration;
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use entity_rules::sheep::flock;
use simulation::simulation_api::entity_reads_and_writes::EntitiesBetweenTicks;
use entity_manager::{WorldEntities, EntityType};
use simulation::{Halos, Hot, Simulation};
use std::collections::BTreeMap;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;
use base_types::COLLISION;
use protocol::default_types::new_type_registry_with_default_types;
use simulation::simulation_api::RuleCode;
use type_registry::FrozenTypes;
use utilities::rng::RandomSource;

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

/// A world: its superchunks hot about the entities that keep a halo,
/// the rest cold.
pub struct World {
    /// What the world is, and the tick it is at.
    pub(crate) world_info: WorldInfo,
    /// How its superchunks are generated.
    pub(crate) world_generation: WorldGeneration,
    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub(crate) bitmap_arena: BitmapArena,
    /// Every superchunk ever made, as stored: the cold ones' cells, and
    /// the hot ones' as last written back.
    pub(crate) chunk_storage: ChunkStorage,
    /// The hot superchunks' entities, at its tick.
    pub(crate) entity_store: WorldEntities,
    /// Its simulation: the hot superchunks' random numbers.
    pub(crate) world_simulation: Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it ([`saved::encode_state`]).
    pub(crate) cold: BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its halos: the superchunks warming and cooling, and the jobs
    /// making them (`simulation::halos`).
    pub(crate) halos: Halos,
    /// The types it runs with: frozen as it starts, the same as long as
    /// it runs.
    pub(crate) frozen_types: Arc<FrozenTypes<RuleCode>>,
    /// Its rules, of its types, in the order a turn runs them.
    pub(crate) rules: Arc<[rules::Rule]>,
}

impl World {
    /// A world with nothing in it, at `world_info`'s tick: what generating and
    /// loading start from. Hot as `world_info` says ([`hot_of`]). On
    /// `thread_count` threads, every one the machine has if none is given;
    /// of `frozen_types`, ticking `rules`, worked out of them.
    fn empty_world(world_info: WorldInfo, world_generation: WorldGeneration, thread_count: Option<usize>, frozen_types: Arc<FrozenTypes<RuleCode>>, rules: Arc<[Rule]>) -> Self {
        let entity_store = WorldEntities::for_types(&frozen_types, world_info.tick);
        // The world's superchunks not counted, as it grows: one set of threads, the tick's and chunk storage's jobs' alike.
        let dispatcher = Arc::new(thread_count.map_or_else(Dispatcher::of_the_machine, Dispatcher::new));
        let world_simulation = Simulation::on_dispatcher(Arc::clone(&dispatcher));
        let hot = hot_of(&world_info);
        let mut chunk_storage = ChunkStorage::new(1 << 16);
        chunk_storage.page_under(paging_folder(), COLD_POOL_BYTES_KEPT);
        Self {
            world_info,
            world_generation,
            bitmap_arena: BitmapArena::new(),
            chunk_storage,
            entity_store,
            world_simulation,
            cold: BTreeMap::new(),
            halos: Halos::new(hot, dispatcher),
            frozen_types,
            rules,
        }
    }

    /// The types it runs with.
    pub fn frozen_types(&self) -> &FrozenTypes<RuleCode> {
        &self.frozen_types
    }

    /// The place among its rules of the rule whose type's ID is `rule_id`.
    ///
    /// # Panics
    /// If it has no rule of that ID.
    pub fn rule_place(&self, rule_id: u64) -> RulePlace {
        place_of(&self.rules, rule_id).unwrap_or_else(|| panic!("no rule of ID {rule_id:016x}"))
    }
}

/// The types a world made new runs with: the base game's, frozen.
pub fn default_frozen_types() -> Arc<FrozenTypes<RuleCode>> {
    Arc::new(new_type_registry_with_default_types().freeze())
}

/// Which of a world's superchunks are hot, as its `world_info` defines it
/// for the simulation to keep: every one, if it has a size and is
/// forced hot; else those about the entities of the kind it names
/// ([`HOT_ENTITY`] if it names none) -- and, if its camera loads
/// superchunks, the viewport's, however many.
fn hot_of(world_info: &WorldInfo) -> Hot {
    match (world_info.side_in_superchunks, world_info.forced_hot) {
        (Some(side_superchunks), true) => Hot::Forced { world_side_superchunks: side_superchunks },
        (side_if_sized, _) => Hot::AboutHotEntities { entity: world_info.hot_entity.map_or(HOT_ENTITY, EntityType), world_side_superchunks: side_if_sized, viewport: world_info.camera_flock.is_some() },
    }
}

impl World {
    /// Queues a flock of `sheep_per_flock` on `superchunk`, hot, drawn from a
    /// random stream of its own: the same flock whenever it is put
    /// there. Put in the world by `Entities::apply`. Each on a cell
    /// the collision plane does not hold, and set there at once
    /// (`docs/server.md`, "The collision plane").
    pub(crate) fn add_flock(&mut self, superchunk: SuperchunkIndex, sheep_per_flock: usize) {
        let bitmap_arena = &self.bitmap_arena;
        let cell_free = |candidate_cell| bitmap_arena.layer_holds_cell(COLLISION, candidate_cell) != Ok(true);
        let mut entities_between = EntitiesBetweenTicks::from_entities(&mut self.entity_store).where_free(&cell_free);
        flock(&mut entities_between, superchunk, sheep_per_flock, &mut RandomSource::for_stream(!self.world_info.seed, superchunk.0));
        let stood_on: Vec<Write> = entities_between.cells_added().iter().map(|&added_cell| Write::cell(added_cell, WriteOp::SetCell)).collect();
        self.write_cells(COLLISION, stood_on);
    }
}

/// Runs what `clean_parameters` ask for: the server's one way in from
/// a command line, whichever command is nested in them.
pub fn start(clean_parameters: &protocol::server_parameters::CleanServerParameters) -> Result<String, String> {
    if let Some(new_parameters) = clean_parameters.nested(params::NEW) {
        return commands::new(utilities::seed::given_or_fresh_seed(clean_parameters.hex(params::SEED)), new_parameters);
    }
    if let Some(run_parameters) = clean_parameters.nested(params::RUN) {
        return commands::run_saved_world(run_parameters);
    }
    if let Some(info_parameters) = clean_parameters.nested(params::INFO) {
        return commands::describe_saved_world(info_parameters);
    }
    Err("no command given: --help says them".to_string())
}

/// A world as `options` say: generated, its sheep put on and their
/// halos hot before it ticks -- or, forced hot, all of it
/// (`docs/server.md`, "Made from a seed").
pub fn start_world(options: WorldStart) -> World {
    let mut world = generate_sized(options.world_generation, options.seed, options.world_size, options.hot_entity, options.thread_count);
    let forced_hot = matches!(options.world_size, WorldSize::SquareOfSide { forced_hot: true, .. });
    world.world_info.camera_flock = (options.camera_loads && !forced_hot).then_some(options.sheep_per_flock as u64);
    world.halos.hot = hot_of(&world.world_info);
    let every_superchunk = world.halos.hot.all_superchunks();
    if options.sheep_per_flock == 0 {
        // No sheep: nothing hot, unless all of it is forced so.
        if forced_hot {
            world.keep_hot(&every_superchunk);
        }
        return world;
    }
    let flock_superchunks = if every_superchunk.is_empty() { vec![WORLD_MIDDLE] } else { every_superchunk };
    let mut world = flocked_world(world, &flock_superchunks, options.sheep_per_flock);
    if world.world_info.camera_flock.is_some() {
        // The halo about the starting flock is generated before the camera has seen anything: owed its flocks.
        let owed_superchunks: Vec<SuperchunkIndex> = world.entity_store.superchunks().iter().map(|superchunk_entities| superchunk_entities.superchunk_index()).filter(|superchunk| !flock_superchunks.contains(superchunk)).collect();
        world.owe_camera_flocks(&owed_superchunks);
    }
    world
}

/// A world of `seed` with nothing in it yet, nothing hot: what
/// [`start_world`] is built from, and what a way of generating is tried out
/// on by itself (`docs/reference.md`, "Generation").
pub fn generate_sized(world_generation: WorldGeneration, seed: u64, world_size: WorldSize, hot_entity: EntityType, thread_count: Option<usize>) -> World {
    let (side_superchunks, forced_hot) = match world_size {
        WorldSize::NoEnd => (None, false),
        WorldSize::SquareOfSide { side_superchunks: side, forced_hot: forced } => (Some(side), forced),
    };
    // A world made new is of the default types: a world loaded, of its own.
    let frozen_types = default_frozen_types();
    let rules = rules_of(&frozen_types).expect("no more default rules than a world may have");
    World::empty_world(WorldInfo { seed, tick: 0, layers: frozen_types.layer_types(), side_in_superchunks: side_superchunks, forced_hot, hot_entity: Some(hot_entity.0), camera_flock: None, without_camera_flock: Vec::new(), generation_numbers: world_generation.numbers() }, world_generation, thread_count, frozen_types, rules)
}

/// `world`, nothing in it yet, with a flock of `sheep_per_flock` on each of
/// `superchunks` and the halos about them hot, as far as it reaches:
/// what [`start_world`] is built from, and what a flock is tried out on by
/// itself.
pub fn flocked_world(mut world: World, superchunks: &[SuperchunkIndex], sheep_per_flock: usize) -> World {
    let mut flock_superchunks = superchunks.to_vec();
    flock_superchunks.sort_unstable();
    world.keep_hot(&flock_superchunks);
    for &superchunk in &flock_superchunks {
        world.add_flock(superchunk, sheep_per_flock);
    }
    world.entity_store.apply();
    let halo = halos_about(flock_superchunks.iter().copied());
    world.keep_hot(&halo);
    world
}
