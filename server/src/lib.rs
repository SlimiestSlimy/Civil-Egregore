//! Civil Egregore's server: the world as a whole, held by the one crate that
//! puts the rest together -- which a renderer, or the program, is a
//! client of. The world made from a seed ([`generate`]), ticked
//! -- its cells' rules (`mc_rules/`) and its entities (`entity_rules/`)
//! together, the superchunks hot only about the entities that keep a
//! halo ([`halos`]) -- saved ([`save`]) and loaded ([`load`]) as it
//! was, to the cell and the random number. Where a save's files are
//! and what they hold: `chunk_storage::disk`. The design:
//! `docs/server.md`; function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod commands;
pub mod diagnostics;
pub mod halos;
pub mod host;
mod tick;
pub mod transient_data;
mod world_start;

pub use halos::HOT_ENTITY;
pub use simulation::hot::about;
pub use simulation::{HaloChange, COOL_TICKS, WARM_TICKS};
pub use tick::{tick_rules, TickCounts, WorldTick};
pub use world_start::{drawn_seed, Size, Start, FLOCK};

use bitplane_manager::BitmapArena;
use chunk_storage::disk::{self, DiskError, HotSuperchunks, WorldInfo};
use chunk_storage::{ChunkMaps, ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use worldgen::{Generation, Terrain, WALLS, WET};
use chunk_storage::mock::GRASS;
use coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, WORLD_MIDDLE};
use mc_rules::trees::{OLDEST, TREE, TREE_STAGE};
use entity_rules::sheep::flock;
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

/// A world: its superchunks hot about the entities that keep a halo,
/// the rest cold.
pub struct World {
    /// What the world is, and the tick it is at.
    pub info: WorldInfo,
    /// How its superchunks are generated.
    pub generation: Generation,
    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub arena: BitmapArena,
    /// Every superchunk ever made, as stored: the cold ones' cells, and
    /// the hot ones' as last written back.
    pub storage: ChunkStorage,
    /// The hot superchunks' entities, at its tick.
    pub entities: Entities,
    /// Its simulation: the hot superchunks' random numbers.
    pub simulation: Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it ([`saved::encode_state`]).
    pub cold: BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its halos: the superchunks warming and cooling, and the jobs
    /// making them (`simulation::halos`).
    pub halos: Halos,
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
        Self {
            info,
            generation,
            arena: BitmapArena::new(),
            storage: ChunkStorage::new(1 << 16),
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
        flock(&mut self.entities, superchunk, sheep, &mut Rng::for_stream(!self.info.seed, superchunk.0));
    }
}

/// Every layer type a world has: the grass's, the trees', the water's
/// and the walls'.
/// Dirt has none: it is a cell with nothing on it.
fn layer_types() -> Vec<LayerType> {
    [GRASS, TREE, TREE_STAGE.layer_type(), WET].into_iter().chain(WALLS.map(|(layer_type, _)| layer_type)).collect()
}

/// A world as `options` say: generated as they say, its sheep put on
/// and their halos hot before it ticks -- or, forced hot, all of it.
/// Every superchunk -- these, and those made as a flock wanders -- is
/// its terrain, heights and the walls they make, and on it grass and
/// trees in patches. Each from the seed and where it is
/// ([`generate_image`]). If its camera loads superchunks, and it is
/// not forced hot, the viewport's superchunks are hot too, and it keeps
/// how many sheep a superchunk generated in the viewport starts with.
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
    flocked(world, &on, options.sheep)
}

/// A world of `seed` with nothing in it yet, nothing hot, whose
/// superchunks are generated as `generation` says, as far as `size`
/// lets it reach, hot about the entities of the kind `hot_entity`
/// unless forced hot, on `threads` threads, every one the machine has
/// if none is given: what [`start`] is built from, and what a way of
/// generating is tried out on by itself.
pub fn generate_sized(generation: Generation, seed: u64, size: Size, hot_entity: EntityType, threads: Option<usize>) -> World {
    let (side, forced) = match size {
        Size::Unlimited => (None, false),
        Size::Limited { side, forced } => (Some(side), forced),
    };
    World::empty(WorldInfo { seed, tick: 0, layers: layer_types(), side, forced, hot_entity: Some(hot_entity.0), camera_flock: None, generation: generation.numbers() }, generation, threads)
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

/// The image of `superchunk` in a world made from `seed` as
/// `generation` says: its terrain, the ocean where it is under the
/// ocean's level, and on the rest grass in patches -- dirt where there is none --
/// and trees in patches of their own, each of a stage drawn for its
/// cell -- the same whenever it is made.
pub(crate) fn generate_image(generation: &Generation, seed: u64, superchunk: SuperchunkIndex, codec: &mut LayerCodec) -> SuperchunkImage {
    let terrain = Terrain::generate_shaped(&generation.shape, seed, superchunk);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let growth = generation.growth(seed);
    // The planes generated, each a bitmap a chunk: grass, trees, their stage's four, and the cells under water.
    let mut planes = vec![GRASS, TREE];
    planes.extend(TREE_STAGE.layer_type().planes());
    let wet = planes.len();
    planes.push(WET);
    // The ocean wherever the ground is under its level, as deep as it is lower.
    let depth_at = |place: usize| generation.shape.ocean.saturating_sub(terrain.height(place));
    let mut cells = vec![[0u64; bitmap::WORDS]; planes.len() * CHUNKS_IN_SUPERCHUNK];
    for place in 0..CHUNKS_IN_SUPERCHUNK * CELLS_IN_CHUNK {
        let (x, y) = cartesian_from_place(place);
        let (x, y) = (left + x, top + y);
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        let mut set = |plane: usize| cells[plane * CHUNKS_IN_SUPERCHUNK + chunk][cell / bitmap::BITS_PER_WORD] |= 1 << (cell % bitmap::BITS_PER_WORD);
        // Nothing grows under water.
        if depth_at(place) > 0 {
            set(wet);
            continue;
        }
        let grown = growth.at(x, y);
        if grown.grass {
            set(0);
        }
        if let Some(lot) = grown.tree {
            set(1);
            // Its stage: a lot of the cell's own.
            let stage = lot % (OLDEST as u64 + 1);
            (0..TREE_STAGE.layer_type().bits() as usize).filter(|bit| stage >> bit & 1 == 1).for_each(|bit| set(2 + bit));
        }
    }
    // A layer a chunk for each plane with a cell set on it, and for each way's walls.
    let mut layers: Vec<(usize, LayerType, Vec<u64>)> = Vec::new();
    for (index, cells) in cells.iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
        layers.push((index % CHUNKS_IN_SUPERCHUNK, planes[index / CHUNKS_IN_SUPERCHUNK], codec.encode(cells).to_vec()));
    }
    for (way, &(layer_type, _)) in WALLS.iter().enumerate() {
        for (place, cells) in terrain.walls[way].iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
            layers.push((place, layer_type, codec.encode(cells).to_vec()));
        }
    }
    let changes: Vec<LayerChange> = layers.iter().map(|(place, layer_type, words)| LayerChange { place: *place, layer_type: *layer_type, encoded: words }).collect();
    SuperchunkImage::new(&terrain.heights).with_water(&ChunkMaps::from_numbers(depth_at)).rewritten(&changes)
}

/// Saves `world` in `folder`, made if not there -- between two ticks.
/// Every dirty bitmap is written back and the ring flushed first, so
/// the cold pool's images are the world's cells; each superchunk's
/// state is its live one if hot, as kept if cold; and which superchunks
/// are hot, which of them cooling, and which warming, in the hot file.
pub fn save(folder: &Path, world: &mut World) -> Result<Saved, DiskError> {
    let hot = world.arena.superchunk_indices();
    world.write_back_and_flush_all();
    for &superchunk in &hot {
        // One with no cell ever set has no image yet: saved all the same.
        if world.storage.image(superchunk).is_none() {
            world.storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()));
        }
    }
    let mut saved = Saved::default();
    let random: Vec<(SuperchunkIndex, u64)> = world.simulation.random_states().collect();
    for superchunk in world.storage.superchunks() {
        saved.bytes += disk::write_image(folder, superchunk, world.storage.image(superchunk).expect("a superchunk of the cold pool"))?;
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
    // The layers made hot are the code's: a save lists what it was written with.
    let info = WorldInfo { layers: layer_types(), ..disk::read_world(folder)? };
    let hot = disk::read_hot(folder)?;
    let generation = Generation::of_numbers(&info.generation);
    let mut world = World::empty(info, generation, None);
    let mut kept = 0;
    for superchunk in disk::saved_superchunks(folder)? {
        world.storage.insert(superchunk, disk::read_image(folder, superchunk)?);
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
