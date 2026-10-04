//! TileSim's world as a whole: made from a seed ([`generate`]), ticked
//! -- its cells' rules (`mc_rules/`) and its entities (`entity_rules/`)
//! together, the superchunks hot only about the entities that keep a
//! halo ([`halos`]) -- saved ([`save`]) and loaded ([`load`]) as it
//! was, to the cell and the random number. Where a save's files are
//! and what they hold: `chunk_storage::disk`. The design:
//! `docs/world.md`; function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
pub mod halos;
mod tick;
pub mod transient_data;

pub use halos::{HaloChange, HALO_KEEPERS};
pub use tick::{tick_rules, TickCounts, WorldTick};

use bitplane_manager::BitmapArena;
use chunk_storage::disk::{self, DiskError, WorldInfo};
use chunk_storage::{ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use terrain::{Terrain, WALLS};
use chunk_storage::mock::{grass_on_dirt, DIRT, GRASS};
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use entity_rules::sheep::flock;
use simulation::entity_store::{saved, Entities};
use simulation::Simulation;
use std::collections::BTreeMap;
use std::path::Path;
use utilities::rng::Rng;

/// Cells of grass drawn on a superchunk generated: a third of it, less those drawn twice.
const GRASS_CELLS: usize = 400_000;
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
    /// The codec that encodes and decodes its layers.
    codec: LayerCodec,
}

impl World {
    /// A world with nothing in it, at `info`'s tick: what generating and
    /// loading start from.
    fn empty(info: WorldInfo) -> Self {
        let entities = Entities::at_tick(info.tick);
        // Every thread the machine has: the world's superchunks are not counted, as it grows.
        let simulation = Simulation::for_superchunks(usize::MAX);
        Self { info, arena: BitmapArena::new(), storage: ChunkStorage::new(1 << 16), entities, simulation, cold: BTreeMap::new(), codec: LayerCodec::new() }
    }
}

/// Every layer type a world has: the pasture's and the walls'.
fn layer_types() -> Vec<LayerType> {
    [DIRT, GRASS].into_iter().chain(WALLS.map(|(layer_type, _)| layer_type)).collect()
}

/// A world made from `seed`: its origin superchunk ([`WORLD_MIDDLE`])
/// with a flock of `sheep` on it, and the superchunks of their halo
/// about it. Every superchunk -- these, and those made as the sheep
/// wander -- is its terrain, heights and the walls they make, and on
/// it, for now, pasture: dirt, a third of it grass. Each from the seed
/// and where it is ([`generate_image`]).
pub fn generate(seed: u64, sheep: usize) -> World {
    let mut world = World::empty(WorldInfo { name: String::new(), seed, tick: 0, layers: layer_types() });
    world.keep_hot(&[WORLD_MIDDLE]);
    flock(&mut world.entities, WORLD_MIDDLE, sheep, &mut Rng::for_stream(!seed, WORLD_MIDDLE.0));
    world.entities.apply();
    world.move_halos();
    world
}

/// The image of `superchunk` in a world made from `seed`: its terrain,
/// and pasture on it -- the same whenever it is made.
fn generate_image(seed: u64, superchunk: SuperchunkIndex, codec: &mut LayerCodec) -> SuperchunkImage {
    let own = Rng::for_stream(seed, superchunk.0).draw();
    let terrain = Terrain::generate(seed, superchunk);
    // Each way's walls, a layer a chunk that has any.
    let mut walls: Vec<(usize, LayerType, Vec<u64>)> = Vec::new();
    for (way, &(layer_type, _)) in WALLS.iter().enumerate() {
        for (place, cells) in terrain.walls[way].iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
            walls.push((place, layer_type, codec.encode(cells).to_vec()));
        }
    }
    let changes: Vec<LayerChange> = walls.iter().map(|(place, layer_type, words)| LayerChange { place: *place, layer_type: *layer_type, encoded: words }).collect();
    grass_on_dirt(own, GRASS_CELLS, codec).with_heights(&terrain.heights).rewritten(&changes)
}

/// Saves `world` in `folder`, made if not there -- between two ticks.
/// Every dirty bitmap is written back and the ring flushed first, so
/// the cold pool's images are the world's cells; each superchunk's
/// state is its live one if hot, as kept if cold.
pub fn save(folder: &Path, world: &mut World) -> Result<Saved, DiskError> {
    let hot = world.arena.superchunk_indices();
    for &superchunk in &hot {
        world.arena.write_back(superchunk, &mut world.storage, &mut world.codec);
    }
    let mut flushed = Vec::new();
    world.storage.flush_all(&mut flushed);
    world.arena.flushed(&flushed);
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
            Some(words) => (words.clone(), saved::entity_count(words)),
            None => {
                let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
                saved::encode_state(state, world.entities.superchunk(superchunk))
            }
        };
        saved.bytes += disk::write_state(folder, superchunk, &words)?;
        saved.superchunks += 1;
        saved.entities += count;
    }
    // The world's file last: a save cut short leaves the one before it.
    saved.bytes += disk::write_world(folder, &WorldInfo { tick: world.entities.now(), ..world.info.clone() })?;
    Ok(saved)
}

/// Loads the world saved in `folder`: every superchunk's image into the
/// cold pool and its state kept as a cold one's, then the halos about
/// the entities that keep one made hot -- as it was when saved, to the
/// cell and the random number.
pub fn load(folder: &Path) -> Result<World, DiskError> {
    let info = disk::read_world(folder)?;
    let mut world = World::empty(info);
    let mut keepers = Vec::new();
    for superchunk in disk::saved_superchunks(folder)? {
        world.storage.insert(superchunk, disk::read_image(folder, superchunk)?);
        let (words, path) = disk::read_state(folder, superchunk)?;
        if saved::holds_any(&words, &HALO_KEEPERS).map_err(|what| DiskError::Invalid(path, what.to_string()))? {
            keepers.push(superchunk);
        }
        world.cold.insert(superchunk, words);
    }
    let kept: usize = world.cold.values().map(|words| saved::entity_count(words)).sum();
    world.keep_hot(&halos::about(keepers.into_iter()));
    let held = world.entities.len() + world.cold.values().map(|words| saved::entity_count(words)).sum::<usize>();
    if held != kept {
        return Err(DiskError::Invalid(folder.to_path_buf(), format!("{held} of {kept} entities put back: some on a cell taken")));
    }
    Ok(world)
}
