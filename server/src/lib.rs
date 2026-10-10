//! Civil Egregore's server: the world as a whole, held by the one
//! crate that puts the rest together -- made from a seed ([`start`]),
//! ticked, saved ([`save`]) and loaded ([`load`]) as it was. A
//! renderer, or the program, is a client of it.
//!
//! The design: `docs/server.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod commands;
pub mod diagnostics;
pub mod halos;
pub mod host;
pub mod rules;
mod tick;
mod world_access;
pub mod transient_data;
mod world_hash;
mod world_start;

pub use halos::HOT_ENTITY;
pub use simulation::hot::about;
pub use simulation::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use rules::{Chosen, Rule, RulePlace, TickCounts, GRASS_RULE, RULES, SHEEP_RULE, TREES_RULE};
pub use tick::WorldTick;
pub use world_hash::{world_hash, WorldHash};
pub use world_start::{drawn_seed, seed_with_land, Size, Start, FLOCK};

use bitplane_manager::BitmapArena;
use chunk_storage::disk::{self, DiskError, HotSuperchunks, WorldInfo};
use chunk_storage::{ChunkStorage, HeightMap, SuperchunkImage};
pub use worldgen::Generation;
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use entity_rules::sheep::flock;
use instructions::entities::EntitiesBetweenTicks;
use entity_manager::{saved, Entities, EntityType};
use simulation::{Halos, Hot, Simulation};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;
use utilities::rng::Rng;

/// What a save wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    /// Superchunks written, hot and cold.
    pub superchunks: usize,
    /// Entities written.
    pub entities: usize,
    /// Bytes written, every file.
    pub bytes: u64,
}

/// Bytes of cold superchunks' images a world keeps in memory, unless
/// told another number ([`World::keep_cold_pool_within`]): past it
/// they are paged to disk (`docs/server.md`, "The cold pool paged").
pub const COLD_POOL_BYTES_KEPT: u64 = 2 << 30;

/// A world: its superchunks hot about the entities that keep a halo,
/// the rest cold.
pub struct World {
    /// What the world is, and the tick it is at.
    pub(crate) info: WorldInfo,
    /// How its superchunks are generated.
    pub(crate) generation: Generation,
    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub(crate) arena: BitmapArena,
    /// Every superchunk ever made, as stored: the cold ones' cells, and
    /// the hot ones' as last written back.
    pub(crate) storage: ChunkStorage,
    /// The hot superchunks' entities, at its tick.
    pub(crate) entities: Entities,
    /// Its simulation: the hot superchunks' random numbers.
    pub(crate) simulation: Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it ([`saved::encode_state`]).
    pub(crate) cold: BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its halos: the superchunks warming and cooling, and the jobs
    /// making them (`simulation::halos`).
    pub(crate) halos: Halos,
}

impl World {
    /// A world with nothing in it, at `info`'s tick: what generating and
    /// loading start from. Hot as `info` says ([`hot_of`]). On
    /// `threads` threads, every one the machine has if none is given.
    fn empty(info: WorldInfo, generation: Generation, threads: Option<usize>) -> Self {
        let entities = Entities::at_tick(info.tick);
        // The world's superchunks not counted, as it grows: one set of threads, the tick's and chunk storage's jobs' alike.
        let dispatcher = Arc::new(threads.map_or_else(Dispatcher::of_the_machine, Dispatcher::new));
        let simulation = Simulation::on(Arc::clone(&dispatcher));
        let hot = hot_of(&info);
        let mut storage = ChunkStorage::new(1 << 16);
        storage.page_under(transient_data::paging(), COLD_POOL_BYTES_KEPT);
        Self {
            info,
            generation,
            arena: BitmapArena::new(),
            storage,
            entities,
            simulation,
            cold: BTreeMap::new(),
            halos: Halos::new(hot, dispatcher),
        }
    }
}

/// Which of a world's superchunks are hot, as its `info` defines it
/// for the simulation to keep: every one, if it has a size and is
/// forced hot; else those about the entities of the kind it names
/// ([`HOT_ENTITY`] if it names none) -- and, if its camera loads
/// superchunks, the viewport's, however many.
fn hot_of(info: &WorldInfo) -> Hot {
    match (info.side, info.forced) {
        (Some(side), true) => Hot::Forced { side },
        (side, _) => Hot::About { entity: info.hot_entity.map_or(HOT_ENTITY, EntityType), side, viewport: info.camera_flock.is_some() },
    }
}

impl World {
    /// Queues a flock of `sheep` on `superchunk`, hot, drawn from a
    /// random stream of its own: the same flock whenever it is put
    /// there. Put in the world by `Entities::apply`.
    pub(crate) fn put_flock(&mut self, superchunk: SuperchunkIndex, sheep: usize) {
        flock(&mut EntitiesBetweenTicks::of(&mut self.entities), superchunk, sheep, &mut Rng::for_stream(!self.info.seed, superchunk.0));
    }
}

/// A world as `options` say: generated, its sheep put on and their
/// halos hot before it ticks -- or, forced hot, all of it
/// (`docs/server.md`, "Made from a seed").
pub fn start(options: Start) -> World {
    let mut world = generate_sized(options.generation, options.seed, options.size, options.hot_entity, options.threads);
    let forced = matches!(options.size, Size::Limited { forced: true, .. });
    world.info.camera_flock = (options.camera_loads && !forced).then_some(options.sheep as u64);
    world.halos.hot = hot_of(&world.info);
    let everywhere = world.halos.hot.all();
    if options.sheep == 0 {
        // No sheep: nothing hot, unless all of it is forced so.
        if forced {
            world.keep_hot(&everywhere);
        }
        return world;
    }
    let on = if everywhere.is_empty() { vec![WORLD_MIDDLE] } else { everywhere };
    let mut world = flocked(world, &on, options.sheep);
    if world.info.camera_flock.is_some() {
        // The halo about the starting flock is generated before the camera has seen anything: owed its flocks.
        let owed: Vec<SuperchunkIndex> = world.entities.superchunks().iter().map(|kept| kept.index()).filter(|superchunk| !on.contains(superchunk)).collect();
        world.owe_camera_flocks(&owed);
    }
    world
}

/// A world of `seed` with nothing in it yet, nothing hot: what
/// [`start`] is built from, and what a way of generating is tried out
/// on by itself (`docs/reference.md`, "Generation").
pub fn generate_sized(generation: Generation, seed: u64, size: Size, hot_entity: EntityType, threads: Option<usize>) -> World {
    let (side, forced) = match size {
        Size::Unlimited => (None, false),
        Size::Limited { side, forced } => (Some(side), forced),
    };
    World::empty(WorldInfo { seed, tick: 0, layers: type_registry::layer_types(), side, forced, hot_entity: Some(hot_entity.0), camera_flock: None, without_camera_flock: Vec::new(), generation: generation.numbers() }, generation, threads)
}

/// `world`, nothing in it yet, with a flock of `sheep` on each of
/// `superchunks` and the halos about them hot, as far as it reaches:
/// what [`start`] is built from, and what a flock is tried out on by
/// itself.
pub fn flocked(mut world: World, superchunks: &[SuperchunkIndex], sheep: usize) -> World {
    let mut flocked = superchunks.to_vec();
    flocked.sort_unstable();
    world.keep_hot(&flocked);
    for &superchunk in &flocked {
        world.put_flock(superchunk, sheep);
    }
    world.entities.apply();
    let halo = about(flocked.iter().copied());
    world.keep_hot(&halo);
    world
}

/// Saves `world` in `folder`, made if not there -- between two ticks.
/// An image paged to disk is copied there file to file, never read
/// into memory.
/// Every dirty bitmap is written back and the ring flushed first, so
/// the cold pool's images are the world's cells; each superchunk's
/// state is its live one if hot, as kept if cold; and which superchunks
/// are hot, which of them cooling, and which warming, in the hot file.
pub fn save(folder: &Path, world: &mut World) -> Result<Saved, DiskError> {
    let hot = world.arena.superchunk_indices();
    world.write_back_and_flush_all();
    for &superchunk in &hot {
        // One with no cell ever set has no image yet: saved all the same.
        if !world.storage.holds(superchunk) {
            world.storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()));
        }
    }
    let mut saved = Saved::default();
    let random: Vec<(SuperchunkIndex, u64)> = world.simulation.random_states().collect();
    for superchunk in world.storage.superchunks() {
        saved.bytes += world.storage.save_image(folder, superchunk)?;
        let (words, count) = match world.cold.get(&superchunk) {
            Some(words) => (words.clone(), saved::entity_count(words).expect("a cold superchunk's state, as it was kept")),
            None => {
                let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
                saved::encode_state(state, world.entities.superchunk(superchunk))
            }
        };
        saved.bytes += disk::write_state(folder, superchunk, &words)?;
        saved.superchunks += 1;
        saved.entities += count;
    }
    saved.bytes += disk::write_hot(folder, &HotSuperchunks { hot, cooling: world.cooling().collect(), warming: world.warming().collect() })?;
    // The world's file last: a save cut short leaves the one before it.
    // The layers a save lists are those it holds: a wide plane's, a bit each.
    let layers = world.info.layers.iter().flat_map(|layer| layer.planes()).collect();
    saved.bytes += disk::write_world(folder, &WorldInfo { tick: world.entities.now(), layers, generation: world.generation.numbers(), ..world.info.clone() })?;
    Ok(saved)
}

/// The worlds saved in `folder`, by their folders' names, sorted:
/// each of its folders a world's file is read from. None if `folder`
/// is not there.
pub fn worlds_in(folder: &Path) -> Vec<String> {
    let Ok(folders) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut worlds: Vec<String> = folders.flatten().filter(|within| disk::read_world(&within.path()).is_ok()).map(|within| within.file_name().to_string_lossy().into_owned()).collect();
    worlds.sort_unstable();
    worlds
}

/// Loads the world saved in `folder`: every superchunk's image into the
/// cold pool and its state kept as a cold one's; then the superchunks
/// hot when it was saved made hot, before it ticks, those cooling
/// cooling again and those warming warming again, each to turn when it
/// was to -- as it was when saved, to the cell and the random number.
pub fn load(folder: &Path) -> Result<World, DiskError> {
    load_keeping(folder, COLD_POOL_BYTES_KEPT)
}

/// [`load`], the world keeping `bytes_kept` bytes of its cold
/// superchunks' images in memory: every image is read and checked,
/// and those past that many bytes are left in `folder`, read again
/// when wanted -- so the save is not to be removed while the world
/// runs.
pub fn load_keeping(folder: &Path, bytes_kept: u64) -> Result<World, DiskError> {
    // The layers made hot are the code's: a save lists what it was written with.
    let info = WorldInfo { layers: type_registry::layer_types(), ..disk::read_world(folder)? };
    let hot = disk::read_hot(folder)?;
    let generation = Generation::of_numbers(&info.generation);
    let mut world = World::empty(info, generation, None);
    world.storage.keep_in_memory(bytes_kept);
    let (mut kept, saved_in) = (0, Arc::<Path>::from(folder));
    for superchunk in disk::saved_superchunks(folder)? {
        let image = disk::read_image(folder, superchunk)?;
        match world.storage.over_memory_kept() {
            true => world.storage.insert_on_disk(superchunk, &saved_in),
            false => world.storage.insert(superchunk, image),
        }
        let (words, path) = disk::read_state(folder, superchunk)?;
        // Every state read whole now: it is decoded only when its superchunk turns hot.
        kept += saved::entity_count(&words).map_err(|what| DiskError::Invalid(path, what.to_string()))?;
        world.cold.insert(superchunk, words);
    }
    world.keep_hot(&hot.hot);
    world.halos.restore_cooling(hot.cooling);
    for (superchunk, due) in hot.warming {
        world.start_warming(superchunk, due);
    }
    let held = world.entities.len() + world.cold.values().map(|words| saved::entity_count(words).expect("read whole")).sum::<usize>();
    if held != kept {
        return Err(DiskError::Invalid(folder.to_path_buf(), format!("{held} of {kept} entities put back: some on a cell taken")));
    }
    Ok(world)
}
