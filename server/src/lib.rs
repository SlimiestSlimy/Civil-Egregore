//! TileSim's server: the world as a whole, held by the one crate that
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
mod tick;
pub mod transient_data;

pub use halos::{HaloChange, COOL_TICKS, HALO_KEEPERS, WARM_TICKS};
pub use tick::{tick_rules, TickCounts, WorldTick};

use chunk_storage::jobs::{Jobs, Ticket};
use bitplane_manager::BitmapArena;
use chunk_storage::disk::{self, DiskError, HotSuperchunks, WorldInfo};
use chunk_storage::{ChunkMaps, ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use worldgen::{Shape, Terrain, WALLS, WET};
use chunk_storage::mock::GRASS;
use coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, WORLD_MIDDLE};
use mc_rules::trees::{OLDEST, TREE, TREE_STAGE};
use worldgen::patches::Patches;
use worldgen::ONE;
use utilities::hash::mix;
use entity_rules::sheep::flock;
use entity_manager::{saved, Entities};
use simulation::Simulation;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;
use utilities::rng::Rng;

/// How superchunks are generated: the heights' shape, and how the
/// grass and the trees lie on them. Not saved with a world: one loaded
/// goes on with [`Generation::DEFAULT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    /// The heights' shape.
    pub shape: Shape,
    /// How the grass lies.
    pub grass: Patches,
    /// How the trees lie.
    pub trees: Patches,
}

impl Generation {
    /// How worlds are generated, as tuned by eye in the renderer's lab.
    pub const DEFAULT: Self = Self {
        shape: Shape::DEFAULT,
        grass: Patches { cover: ONE * 951 / 1000, patch: 8, detail: ONE * 598 / 1000, scatter: ONE * 51 / 1000 },
        trees: Patches { cover: ONE * 60 / 1000, patch: 7, detail: ONE * 800 / 1000, scatter: ONE * 300 / 1000 },
    };
}

/// What keeps the trees' numbers apart from the grass's.
pub const TREES_SALT: u64 = 0x7472_6565_735F_6C6F;
/// Sheep the world's origin superchunk starts with, unless told.
pub const FLOCK: usize = 4_000;

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
    /// The threads encoding, generating and decoding off the tick.
    jobs: Jobs,
    /// The superchunks warming, sorted ([`halos`]).
    warming: Vec<halos::Warming>,
    /// The hot superchunks cooling, sorted, each with the tick it goes
    /// cold at ([`halos`]).
    cooling: Vec<(SuperchunkIndex, u64)>,
    /// The write-backs of superchunks gone cold, each with its job,
    /// being encoded by chunk storage's jobs, in the order taken.
    writing_back: VecDeque<(SuperchunkIndex, Ticket)>,
    /// The superchunks whose changes were taken from the ring, each with
    /// its job, their images being rewritten by a job.
    flushing: Vec<(SuperchunkIndex, Ticket)>,
}

impl World {
    /// A world with nothing in it, at `info`'s tick: what generating and
    /// loading start from.
    fn empty(info: WorldInfo, generation: Generation) -> Self {
        let entities = Entities::at_tick(info.tick);
        // Every thread the machine has, the world's superchunks not counted, as it grows: one set of them, the tick's and chunk storage's jobs' alike.
        let dispatcher = Arc::new(Dispatcher::of_the_machine());
        let simulation = Simulation::on(Arc::clone(&dispatcher));
        Self {
            info,
            generation,
            arena: BitmapArena::new(),
            storage: ChunkStorage::new(1 << 16),
            entities,
            simulation,
            cold: BTreeMap::new(),
            jobs: Jobs::new(dispatcher),
            warming: Vec::new(),
            cooling: Vec::new(),
            writing_back: VecDeque::new(),
            flushing: Vec::new(),
        }
    }
}

/// Every layer type a world has: the grass's, the trees', the water's
/// and the walls'.
/// Dirt has none: it is a cell with nothing on it.
fn layer_types() -> Vec<LayerType> {
    [GRASS, TREE, TREE_STAGE.layer_type(), WET].into_iter().chain(WALLS.map(|(layer_type, _)| layer_type)).collect()
}

/// A world made from `seed`: its origin superchunk ([`WORLD_MIDDLE`])
/// with a flock of `sheep` on it, and the superchunks of their halo
/// about it, all hot before it ticks. Every superchunk -- these, and those made as the sheep
/// wander -- is its terrain, heights and the walls they make, and on
/// it, for now, pasture: dirt, a third of it grass. Each from the seed
/// and where it is ([`generate_image`]).
pub fn generate(seed: u64, sheep: usize) -> World {
    generate_flocks(seed, &[WORLD_MIDDLE], sheep)
}

/// A world of `seed` with nothing in it yet, nothing hot, whose
/// superchunks are generated as `generation` says: what a way of
/// generating is tried out on.
pub fn generate_with(generation: Generation, seed: u64) -> World {
    World::empty(WorldInfo { name: String::new(), seed, tick: 0, layers: layer_types() }, generation)
}

/// The first seed from `from` on whose world, shaped as `shape`, has
/// land about its origin -- three superchunks each way: what a world is
/// made from to be watched or tested with a flock on it, the seed
/// otherwise as likely to give ocean there.
pub fn seed_with_land(from: u64, shape: &Shape) -> u64 {
    let (middle, side) = (WORLD_MIDDLE.top_left().cartesian(), coordinates::SUPERCHUNK_SIDE_CELLS as i32);
    let land = |seed: &u64| {
        let mut lands = worldgen::mesh::Lands::new(shape, *seed);
        (-3i32..=3).all(|across| (-3i32..=3).all(|down| lands.height(middle.x.wrapping_add_signed(across * side), middle.y.wrapping_add_signed(down * side)) > shape.ocean))
    };
    (from..).find(land).expect("a seed with land about the origin")
}

/// A world made from `seed` as [`generate`] makes one, but with a
/// flock of `sheep` on each of `superchunks`, and the halos about them
/// all hot before it ticks.
pub fn generate_flocks(seed: u64, superchunks: &[SuperchunkIndex], sheep: usize) -> World {
    generate_flocks_with(Generation::DEFAULT, seed, superchunks, sheep)
}

/// [`generate_flocks`], in a world generated as `generation` says.
pub fn generate_flocks_with(generation: Generation, seed: u64, superchunks: &[SuperchunkIndex], sheep: usize) -> World {
    let mut world = generate_with(generation, seed);
    let mut flocked = superchunks.to_vec();
    flocked.sort_unstable();
    world.keep_hot(&flocked);
    for &superchunk in &flocked {
        flock(&mut world.entities, superchunk, sheep, &mut Rng::for_stream(!seed, superchunk.0));
    }
    world.entities.apply();
    let halo = halos::about(flocked.iter().copied());
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
    let trees_seed = seed ^ TREES_SALT;
    let (grass_under, trees_under) = (generation.grass.threshold(seed), generation.trees.threshold(trees_seed));
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
        if generation.grass.number(seed, x, y) < grass_under {
            set(0);
        }
        if generation.trees.number(trees_seed, x, y) < trees_under {
            set(1);
            // Its stage: a lot of the cell's own.
            let stage = mix(trees_seed ^ ((y as u64) << 32 | x as u64)) % (OLDEST as u64 + 1);
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
    world.write_back_all();
    world.flush_all();
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
    saved.bytes += disk::write_world(folder, &WorldInfo { tick: world.entities.now(), layers, ..world.info.clone() })?;
    Ok(saved)
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
    let mut world = World::empty(info, Generation::DEFAULT);
    let mut kept = 0;
    for superchunk in disk::saved_superchunks(folder)? {
        world.storage.insert(superchunk, disk::read_image(folder, superchunk)?);
        let (words, path) = disk::read_state(folder, superchunk)?;
        // Every state read whole now: it is decoded only when its superchunk turns hot.
        kept += saved::entity_count(&words).map_err(|what| DiskError::Invalid(path, what.to_string()))?;
        world.cold.insert(superchunk, words);
    }
    world.keep_hot(&hot.hot);
    world.cooling = hot.cooling;
    for (superchunk, due) in hot.warming {
        world.start_warming(superchunk, due);
    }
    let held = world.entities.len() + world.cold.values().map(|words| saved::entity_count(words).expect("read whole")).sum::<usize>();
    if held != kept {
        return Err(DiskError::Invalid(folder.to_path_buf(), format!("{held} of {kept} entities put back: some on a cell taken")));
    }
    Ok(world)
}
