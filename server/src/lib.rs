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
pub use simulation::hot::halos_about;
pub use simulation::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use rules::{ChosenRules, Rule, RulePlace, TickCounts, GRASS_RULE, RULES, SHEEP_RULE, TREES_RULE, WEATHER_RULE};
pub use tick::WorldTick;
pub use world_hash::{world_hash, WorldHash};
pub use world_start::{drawn_seed, seed_with_land, WorldSize, WorldStart, FLOCK};

use bitplane_manager::{BitmapArena, Write, WriteOp};
use chunk_storage::disk::{self, DiskError, HotSuperchunks, WorldInfo};
use chunk_storage::{ChunkStorage, HeightMap, SuperchunkImage};
pub use worldgen::WorldGeneration;
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use entity_rules::sheep::flock;
use actions::entities::EntitiesBetweenTicks;
use entity_manager::{saved, WorldEntities, EntityType};
use simulation::{Halos, Hot, Simulation};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;
use type_registry::COLLISION;
use utilities::rng::RandomSource;

/// What a save wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveWritten {
    /// Superchunks written, hot and cold.
    pub superchunks: usize,
    /// Entities written.
    pub entities_written: usize,
    /// Bytes written, every file.
    pub bytes_written: u64,
}

/// Bytes of cold superchunks' images a world keeps in memory, unless
/// told another number ([`World::keep_cold_pool_within`]): past it
/// they are paged to disk (`docs/server.md`, "The cold pool paged").
pub const COLD_POOL_BYTES_KEPT: u64 = 2 << 30;

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
}

impl World {
    /// A world with nothing in it, at `world_info`'s tick: what generating and
    /// loading start from. Hot as `world_info` says ([`hot_of`]). On
    /// `thread_count` threads, every one the machine has if none is given.
    fn empty_world(world_info: WorldInfo, world_generation: WorldGeneration, thread_count: Option<usize>) -> Self {
        let entity_store = WorldEntities::at_tick(world_info.tick);
        // The world's superchunks not counted, as it grows: one set of threads, the tick's and chunk storage's jobs' alike.
        let dispatcher = Arc::new(thread_count.map_or_else(Dispatcher::of_the_machine, Dispatcher::new));
        let mut world_simulation = Simulation::on_dispatcher(Arc::clone(&dispatcher));
        world_simulation.keep_entities_in(COLLISION);
        let hot = hot_of(&world_info);
        let mut chunk_storage = ChunkStorage::new(1 << 16);
        chunk_storage.page_under(transient_data::paging_folder(), COLD_POOL_BYTES_KEPT);
        Self {
            world_info,
            world_generation,
            bitmap_arena: BitmapArena::new(),
            chunk_storage,
            entity_store,
            world_simulation,
            cold: BTreeMap::new(),
            halos: Halos::new(hot, dispatcher),
        }
    }
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
    pub(crate) fn put_flock(&mut self, superchunk: SuperchunkIndex, sheep_per_flock: usize) {
        let bitmap_arena = &self.bitmap_arena;
        let cell_free = |candidate_cell| bitmap_arena.layer_holds_cell(COLLISION, candidate_cell) != Ok(true);
        let mut entities_between = EntitiesBetweenTicks::from_entities(&mut self.entity_store).where_free(&cell_free);
        flock(&mut entities_between, superchunk, sheep_per_flock, &mut RandomSource::for_stream(!self.world_info.seed, superchunk.0));
        let stood_on: Vec<Write> = entities_between.cells_put().iter().map(|&put_cell| Write::cell(put_cell, WriteOp::SetCell)).collect();
        self.write_cells(COLLISION, stood_on);
    }
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
    World::empty_world(WorldInfo { seed, tick: 0, layers: type_registry::layer_types(), side_in_superchunks: side_superchunks, forced_hot, hot_entity: Some(hot_entity.0), camera_flock: None, without_camera_flock: Vec::new(), generation_numbers: world_generation.numbers() }, world_generation, thread_count)
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
        world.put_flock(superchunk, sheep_per_flock);
    }
    world.entity_store.apply();
    let halo = halos_about(flock_superchunks.iter().copied());
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
pub fn save(folder: &Path, world: &mut World) -> Result<SaveWritten, DiskError> {
    let hot = world.bitmap_arena.superchunk_indices();
    world.write_back_and_flush_all();
    for &superchunk in &hot {
        // One with no cell ever set has no image yet: saved all the same.
        if !world.chunk_storage.holds_image(superchunk) {
            world.chunk_storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()));
        }
    }
    let mut save_written = SaveWritten::default();
    let random_states: Vec<(SuperchunkIndex, u64)> = world.world_simulation.random_states().collect();
    for superchunk in world.chunk_storage.superchunks() {
        save_written.bytes_written += world.chunk_storage.save_image(folder, superchunk)?;
        let (words, count) = match world.cold.get(&superchunk) {
            Some(words) => (words.clone(), saved::entity_count(words).expect("a cold superchunk's state, as it was kept")),
            None => {
                let random_state = random_states.binary_search_by_key(&superchunk, |stream_entry| stream_entry.0).ok().map(|found_entry_index| random_states[found_entry_index].1);
                saved::encode_state(random_state, world.entity_store.superchunk(superchunk))
            }
        };
        save_written.bytes_written += disk::write_state(folder, superchunk, &words)?;
        save_written.superchunks += 1;
        save_written.entities_written += count;
    }
    save_written.bytes_written += disk::write_hot(folder, &HotSuperchunks { hot, cooling: world.cooling().collect(), warming: world.warming().collect() })?;
    // The world's file last: a save cut short leaves the one before it.
    // The layers a save lists are those it holds: a wide plane's, a bit each.
    let layers = world.world_info.layers.iter().flat_map(|layer| layer.each_bitplane()).collect();
    save_written.bytes_written += disk::write_world(folder, &WorldInfo { tick: world.entity_store.current_tick(), layers, generation_numbers: world.world_generation.numbers(), ..world.world_info.clone() })?;
    Ok(save_written)
}

/// The worlds saved in `folder`, by their folders' names, sorted:
/// each of its folders a world's file is read from. None if `folder`
/// is not there.
pub fn worlds_in(folder: &Path) -> Vec<String> {
    let Ok(folders) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut worlds: Vec<String> = folders.flatten().filter(|dir_entry| disk::read_world(&dir_entry.path()).is_ok()).map(|world_entry| world_entry.file_name().to_string_lossy().into_owned()).collect();
    worlds.sort_unstable();
    worlds
}

/// Loads the world saved in `folder`: every superchunk's image into the
/// cold pool and its state kept as a cold one's; then the superchunks
/// hot when it was saved made hot, before it ticks, those cooling
/// cooling again and those warming warming again, each to turn when it
/// was to -- as it was when saved, to the cell and the random number.
pub fn load_world(folder: &Path) -> Result<World, DiskError> {
    load_keeping(folder, COLD_POOL_BYTES_KEPT)
}

/// [`load_world`], the world keeping `bytes_kept` bytes of its cold
/// superchunks' images in memory: every image is read and checked,
/// and those past that many bytes are left in `folder`, read again
/// when wanted -- so the save is not to be removed while the world
/// runs.
pub fn load_keeping(folder: &Path, bytes_kept: u64) -> Result<World, DiskError> {
    // The layers made hot are the code's: a save lists what it was written with.
    let world_info = WorldInfo { layers: type_registry::layer_types(), ..disk::read_world(folder)? };
    let hot = disk::read_hot(folder)?;
    let world_generation = WorldGeneration::of_numbers(&world_info.generation_numbers);
    let mut world = World::empty_world(world_info, world_generation, None);
    world.chunk_storage.keep_in_memory(bytes_kept);
    let (mut entities_kept, saved_in) = (0, Arc::<Path>::from(folder));
    for superchunk in disk::saved_superchunks(folder)? {
        let image = disk::read_image(folder, superchunk)?;
        match world.chunk_storage.over_memory_kept() {
            true => world.chunk_storage.insert_on_disk(superchunk, &saved_in),
            false => world.chunk_storage.insert(superchunk, image),
        }
        let (words, path) = disk::read_state(folder, superchunk)?;
        // Every state read whole now: it is decoded only when its superchunk turns hot.
        entities_kept += saved::entity_count(&words).map_err(|count_error| DiskError::InvalidFile(path, count_error.to_string()))?;
        world.cold.insert(superchunk, words);
    }
    world.keep_hot(&hot.hot);
    world.halos.restore_cooling(hot.cooling);
    for (superchunk, due_tick) in hot.warming {
        world.start_warming(superchunk, due_tick);
    }
    let entities_held = world.entity_store.len() + world.cold.values().map(|words| saved::entity_count(words).expect("read whole")).sum::<usize>();
    if entities_held != entities_kept {
        return Err(DiskError::InvalidFile(folder.to_path_buf(), format!("{entities_held} of {entities_kept} entities put back: some on a cell taken")));
    }
    Ok(world)
}
