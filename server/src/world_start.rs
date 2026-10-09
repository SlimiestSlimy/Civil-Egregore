//! What a new world starts from ([`Start`]): every number it is made
//! with, whoever gives them -- the command line (`commands::new`), or a
//! window's sliders ([`Start::from_tuning`]) -- as the server alone
//! knows what they mean.

use crate::halos::HOT_ENTITY;
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

/// What a world starts from, for [`crate::start`]: `seed`, generated
/// as `generation` says, as far as `size` lets it reach, ticking on
/// `threads` threads, and `sheep` on each superchunk of it -- every one
/// of a world with a size; of one without, the origin's alone, as
/// sheep everywhere would keep the whole of an endless world hot --
/// hot about the entities of the kind `hot_entity`, unless forced hot,
/// their halos hot before it ticks; and, if `camera_loads`, about the
/// superchunks in view too, each generated while in view given
/// `sheep` of its own (`World::keep_in_view`).
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
    /// Whether its camera loads superchunks: those in view hot, and
    /// each generated while in view given `sheep`. Nothing to a world
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
    /// As a window's sliders have it (`utilities::tuning`): how it is
    /// generated, its size -- a side of 0 none, and forced hot counting
    /// only with a side -- whether its camera loads superchunks, which
    /// counts only if it is not forced hot, and its sheep; from `seed`, or if none is
    /// given one drawn at random ([`drawn_seed`]); on every thread the
    /// machine has.
    pub fn from_tuning(seed: Option<u64>, tuning: &Tuning) -> Self {
        let generation = Generation::from_tuning(tuning);
        let side = tuning[WORLD_SIDE].round().max(0.0) as u32;
        let forced = side > 0 && tuning[FORCED_HOT] >= 0.5;
        let size = Size::of_side(side, forced).expect("forced hot only with a side");
        let seed = seed.unwrap_or_else(|| drawn_seed(&generation));
        Self { seed, generation, size, sheep: tuning[SHEEP].max(0.0) as usize, camera_loads: !forced && tuning[CAMERA_LOADS] >= 0.5, ..Self::default() }
    }
}

/// A seed drawn at random: the first from it with land about the
/// origin, where the sheep start, if one is within [`LAND_TRIES`] --
/// else the one drawn.
pub fn drawn_seed(generation: &Generation) -> u64 {
    let drawn = RandomState::new().hash_one(std::time::SystemTime::now());
    (drawn..drawn.saturating_add(LAND_TRIES)).find(|&seed| has_land_about(seed, &generation.shape, WORLD_MIDDLE)).unwrap_or(drawn)
}
