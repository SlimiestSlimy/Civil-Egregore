//! What a new world starts from ([`Start`]): every number it is made
//! with, whoever gives them -- the command line (`commands::new`), or a
//! window's sliders ([`Start::from_tuning`]) -- as the server alone
//! knows what they mean.

use crate::halos::HOT_ENTITY;
use chunk_storage::disk::WorldInfo;
use coordinates::WORLD_MIDDLE;
use entity_manager::EntityType;
use std::hash::{BuildHasher, RandomState};
use utilities::tuning::{Tuning, CAMERA_LOADS, FORCED_HOT, SHEEP, WORLD_SIDE};
use worldgen::{has_land_about, Generation};

/// Sheep the world's origin superchunk starts with, unless told.
pub const FLOCK: usize = 4_000;

/// Seeds tried after one drawn at random for one with land about the
/// origin, where the sheep start, before the one drawn is taken.
const LAND_TRIES: u64 = 256;

/// How far a world reaches, and whether it is hot throughout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// No end: as far as coordinates reach, hot about its hot entities.
    Unlimited,
    /// `side` superchunks along a side, a square about its origin --
    /// nothing outside it ever made or hot -- hot about its hot
    /// entities, or `forced` hot throughout, whatever they do: only a
    /// world with an end can be.
    Limited {
        /// Superchunks along a side.
        side: u32,
        /// Every superchunk of it hot throughout.
        forced: bool,
    },
}

impl Size {
    /// `side` superchunks a side, 0 for no end; forced hot if `forced`,
    /// or why not: only a world with a side can be.
    pub fn of_side(side: u32, forced: bool) -> Result<Self, String> {
        match side {
            0 if forced => Err("only a world with a side can be forced hot".to_string()),
            0 => Ok(Self::Unlimited),
            side => Ok(Self::Limited { side, forced }),
        }
    }
}

/// What a world starts from, for [`crate::start`]: every number,
/// whoever gives it (`docs/server.md`, "Made from a seed").
#[derive(Clone, Copy, Debug)]
pub struct Start {
    /// The seed its superchunks are generated from.
    pub seed: u64,
    /// How its superchunks are generated.
    pub generation: Generation,
    /// How far it reaches, and whether it is forced hot.
    pub size: Size,
    /// Threads it ticks on, every one the machine has if none is given.
    pub threads: Option<usize>,
    /// Sheep each superchunk it puts them on starts with.
    pub sheep: usize,
    /// The kind of entity it is hot about, unless forced hot.
    pub hot_entity: EntityType,
    /// Whether its camera loads superchunks: the viewport's hot, and
    /// each generated in the viewport given `sheep`. Nothing to a world
    /// forced hot, all of which is, nor to one run with no window.
    pub camera_loads: bool,
}

impl Default for Start {
    /// Seed 1, generated as [`Generation::DEFAULT`] says, no size to
    /// it, every thread the machine has, a flock of [`FLOCK`] on its
    /// origin, hot about [`HOT_ENTITY`], its camera loading nothing:
    /// what `new` makes unless told otherwise.
    fn default() -> Self {
        Self { seed: 1, generation: Generation::DEFAULT, size: Size::Unlimited, threads: None, sheep: FLOCK, hot_entity: HOT_ENTITY, camera_loads: false }
    }
}

impl Start {
    /// As a window's sliders have it (`utilities::tuning`), from
    /// `seed` or one drawn ([`drawn_seed`]), on every thread
    /// (`docs/reference.md`, "Start::from_tuning").
    pub fn from_tuning(seed: Option<u64>, tuning: &Tuning) -> Self {
        let generation = Generation::from_tuning(tuning);
        let side = tuning[WORLD_SIDE].round().max(0.0) as u32;
        let forced = side > 0 && tuning[FORCED_HOT] >= 0.5;
        let size = Size::of_side(side, forced).expect("forced hot only with a side");
        let seed = seed.unwrap_or_else(|| drawn_seed(&generation));
        Self { seed, generation, size, sheep: tuning[SHEEP].max(0.0) as usize, camera_loads: !forced && tuning[CAMERA_LOADS] >= 0.5, ..Self::default() }
    }

    /// What the world `info` is of started from, as far as it says: its
    /// seed, generation, size, hot entity and camera -- and the sheep
    /// its camera puts on a superchunk, or if it puts none [`FLOCK`], as
    /// a world's file keeps no other; on every thread the machine has.
    pub fn of_world(info: &WorldInfo, generation: Generation) -> Self {
        let size = Size::of_side(info.side.unwrap_or(0), info.forced).unwrap_or(Size::Unlimited);
        let sheep = info.camera_flock.map_or(FLOCK, |sheep| sheep as usize);
        Self { seed: info.seed, generation, size, sheep, hot_entity: info.hot_entity.map_or(HOT_ENTITY, EntityType), camera_loads: info.camera_flock.is_some(), ..Self::default() }
    }
}

/// The first seed from `from` on whose world, generated as
/// `generation` says, has land about the origin, where the sheep start.
pub fn seed_with_land(from: u64, generation: &Generation) -> u64 {
    worldgen::seed_with_land(from, &generation.shape, WORLD_MIDDLE)
}

/// A seed drawn at random: the first from it with land about the
/// origin, where the sheep start, if one is within [`LAND_TRIES`] --
/// else the one drawn.
pub fn drawn_seed(generation: &Generation) -> u64 {
    let drawn = RandomState::new().hash_one(std::time::SystemTime::now());
    (drawn..drawn.saturating_add(LAND_TRIES)).find(|&seed| has_land_about(seed, &generation.shape, WORLD_MIDDLE)).unwrap_or(drawn)
}
