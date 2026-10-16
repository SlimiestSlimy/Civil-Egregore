//! What a new world starts from ([`WorldStart`]): every number it is made
//! with, whoever gives them -- the command line (`commands::new`), or a
//! window's sliders ([`WorldStart::from_tuning`]) -- as the server alone
//! knows what they mean.

use crate::halos::HOT_ENTITY;
use chunk_storage::disk::WorldInfo;
use coordinates::WORLD_MIDDLE;
use entity_manager::EntityType;
use std::hash::{BuildHasher, RandomState};
use utilities::tuning::{Tuning, CAMERA_LOADS, FORCED_HOT, SHEEP_PER_SUPERCHUNK, WORLD_SIDE};
use worldgen::{has_land_about, WorldGeneration};

/// Sheep the world's origin superchunk starts with, unless told.
pub const FLOCK: usize = 4_000;

/// Seeds tried after one drawn at random for one with land about the
/// origin, where the sheep start, before the one drawn is taken.
const LAND_TRIES: u64 = 256;

/// How far a world reaches, and whether it is hot throughout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSize {
    /// No end: as far as coordinates reach, hot about its hot entities.
    NoEnd,
    /// `side` superchunks along a side, a square about its origin --
    /// nothing outside it ever made or hot -- hot about its hot
    /// entities, or `forced` hot throughout, whatever they do: only a
    /// world with an end can be.
    SquareOfSide {
        /// Superchunks along a side.
        side_superchunks: u32,
        /// Every superchunk of it hot throughout.
        forced_hot: bool,
    },
}

impl WorldSize {
    /// `side_superchunks` superchunks a side, 0 for no end; forced hot if `forced_hot`,
    /// or why not: only a world with a side can be.
    pub fn of_side(side_superchunks: u32, forced_hot: bool) -> Result<Self, String> {
        match side_superchunks {
            0 if forced_hot => Err("only a world with a side can be forced hot".to_string()),
            0 => Ok(Self::NoEnd),
            nonzero_side => Ok(Self::SquareOfSide { side_superchunks: nonzero_side, forced_hot }),
        }
    }
}

/// What a world starts from, for [`crate::start`]: every number,
/// whoever gives it (`docs/server.md`, "Made from a seed").
#[derive(Clone, Copy, Debug)]
pub struct WorldStart {
    /// The seed its superchunks are generated from.
    pub seed: u64,
    /// How its superchunks are generated.
    pub world_generation: WorldGeneration,
    /// How far it reaches, and whether it is forced hot.
    pub world_size: WorldSize,
    /// Threads it ticks on, every one the machine has if none is given.
    pub thread_count: Option<usize>,
    /// Sheep each superchunk it puts them on starts with.
    pub sheep_per_flock: usize,
    /// The kind of entity it is hot about, unless forced hot.
    pub hot_entity: EntityType,
    /// Whether its camera loads superchunks: the viewport's hot, and
    /// each generated in the viewport given `sheep`. Nothing to a world
    /// forced hot, all of which is, nor to one run with no window.
    pub camera_loads: bool,
}

impl Default for WorldStart {
    /// Seed 1, generated as [`Generation::DEFAULT`] says, no size to
    /// it, every thread the machine has, a flock of [`FLOCK`] on its
    /// origin, hot about [`HOT_ENTITY`], its camera loading nothing:
    /// what `new` makes unless told otherwise.
    fn default() -> Self {
        Self { seed: 1, world_generation: WorldGeneration::DEFAULT, world_size: WorldSize::NoEnd, thread_count: None, sheep_per_flock: FLOCK, hot_entity: HOT_ENTITY, camera_loads: false }
    }
}

impl WorldStart {
    /// As a window's sliders have it (`utilities::tuning`), from
    /// `seed` or one drawn ([`drawn_seed`]), on every thread
    /// (`docs/reference.md`, "WorldStart::from_tuning").
    pub fn from_tuning(seed: Option<u64>, tuning: &Tuning) -> Self {
        let world_generation = WorldGeneration::from_tuning(tuning);
        let side_superchunks = tuning[WORLD_SIDE].round().max(0.0) as u32;
        let forced_hot = side_superchunks > 0 && tuning[FORCED_HOT] >= 0.5;
        let world_size = WorldSize::of_side(side_superchunks, forced_hot).expect("forced hot only with a side");
        let seed = seed.unwrap_or_else(|| drawn_seed(&world_generation));
        Self { seed, world_generation, world_size, sheep_per_flock: tuning[SHEEP_PER_SUPERCHUNK].max(0.0) as usize, camera_loads: !forced_hot && tuning[CAMERA_LOADS] >= 0.5, ..Self::default() }
    }

    /// What the world `world_info` is of started from, as far as it says: its
    /// seed, generation, size, hot entity and camera -- and the sheep
    /// its camera puts on a superchunk, or if it puts none [`FLOCK`], as
    /// a world's file keeps no other; on every thread the machine has.
    pub fn of_world(world_info: &WorldInfo, world_generation: WorldGeneration) -> Self {
        let world_size = WorldSize::of_side(world_info.side_in_superchunks.unwrap_or(0), world_info.forced_hot).unwrap_or(WorldSize::NoEnd);
        let sheep_per_flock = world_info.camera_flock.map_or(FLOCK, |camera_sheep| camera_sheep as usize);
        Self { seed: world_info.seed, world_generation, world_size, sheep_per_flock, hot_entity: world_info.hot_entity.map_or(HOT_ENTITY, EntityType), camera_loads: world_info.camera_flock.is_some(), ..Self::default() }
    }
}

/// The first seed from `first_seed_tried` on whose world, generated as
/// `world_generation` says, has land about the origin, where the sheep start.
pub fn seed_with_land(first_seed_tried: u64, world_generation: &WorldGeneration) -> u64 {
    worldgen::seed_with_land(first_seed_tried, &world_generation.land_shape, WORLD_MIDDLE)
}

/// A seed drawn at random: the first from it with land about the
/// origin, where the sheep start, if one is within [`LAND_TRIES`] --
/// else the one drawn.
pub fn drawn_seed(world_generation: &WorldGeneration) -> u64 {
    let random_seed = RandomState::new().hash_one(std::time::SystemTime::now());
    (random_seed..random_seed.saturating_add(LAND_TRIES)).find(|&seed| has_land_about(seed, &world_generation.land_shape, WORLD_MIDDLE)).unwrap_or(random_seed)
}
