//! The lab: the renderer run to tune by eye how the world is made and
//! how it changes -- a world of the superchunks shown, every one hot
//! from the start, no sheep, its rules ticking ([`crate::sim`]). What
//! is here is what the sliders say of generation: the seed, the
//! heights' shape, how the grass and the trees lie. When a slider of
//! generation moves, or the seed is drawn again ([`reseed`]), the
//! world is made afresh and its ticks start from 0.

use crate::sim::SEED;
use crate::tuning::{self, BUMPS, GRASS_COVER, HEIGHT_SPAN, HILLS, PATCH_DETAIL, PATCH_SIZE, RIDGES, ROUGHNESS, SCATTER, TREE_COVER, TREE_DETAIL, TREE_PATCH, TREE_SCATTER, GROUND_LEVEL, LAND_RISE, LAND_SPAN, COAST, OCEAN_LEVEL};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use terrain::Shape;
use utilities::hash::mix;
use world::patches::{Patches, ONE};
use world::Generation;

/// Whether the lab is what runs.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// The seed the world is generated from, now.
static SEED_NOW: AtomicU64 = AtomicU64::new(SEED);

/// Says the lab is what runs: generation is then the sliders'.
pub fn run() {
    RUNNING.store(true, Ordering::Relaxed);
}

/// The seed the world is generated from, now.
pub fn seed() -> u64 {
    SEED_NOW.load(Ordering::Relaxed)
}

/// Draws a new seed, off the clock: the world is generated again.
pub fn reseed() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos() as u64);
    SEED_NOW.store(mix(now), Ordering::Relaxed);
    tuning::regenerate();
}

/// How the world is generated now: as the sliders have it in the lab,
/// as worlds are otherwise.
pub fn generation() -> Generation {
    if !RUNNING.load(Ordering::Relaxed) {
        return Generation::DEFAULT;
    }
    let tuned = tuning::now();
    // What is typed is held to what the noise can take.
    let of_one = |share: f32| (share.clamp(0.0, 1024.0) * ONE as f32) as u64;
    let patches = |[cover, patch, detail, scatter]: [usize; 4]| Patches { cover: of_one(tuned[cover]), patch: tuned[patch].round().clamp(0.0, 16.0) as u32, detail: of_one(tuned[detail]), scatter: of_one(tuned[scatter]) };
    Generation { shape: shape(&tuned), grass: patches([GRASS_COVER, PATCH_SIZE, PATCH_DETAIL, SCATTER]), trees: patches([TREE_COVER, TREE_PATCH, TREE_DETAIL, TREE_SCATTER]) }
}

/// The heights' shape, as the sliders have it: the land, the ocean,
/// and each hill octave's share of the height span, whole numbers that
/// come to no more than the span.
fn shape(tuned: &tuning::Tuning) -> Shape {
    // The hills are a byte high at most, whatever is typed.
    let span = tuned[HEIGHT_SPAN].round().clamp(0.0, 255.0) as u64;
    // The highest ground is still a height: 16 bits.
    let height = |tuned: f32| tuned.round().clamp(0.0, (u16::MAX - 255) as f32) as u16;
    let land = Shape { weights: [0; 4], ocean: height(tuned[OCEAN_LEVEL]), coast: tuned[COAST].round().clamp(1.0, u16::MAX as f32) as u64, ground: height(tuned[GROUND_LEVEL]), rise: height(tuned[LAND_RISE]) as u64, rise_span: tuned[LAND_SPAN].round().clamp(2.0, 24.0) as u32 };
    let shares = [tuned[HILLS], tuned[RIDGES], tuned[BUMPS], tuned[ROUGHNESS]].map(|share| share.max(0.0));
    let all: f32 = shares.iter().sum();
    if all <= 0.0 {
        return land;
    }
    let mut weights = shares.map(|share| (share / all * span as f32).round() as u64);
    // Rounded up together they may pass the span by one: the broadest gives it back.
    weights[0] -= (weights.iter().sum::<u64>().saturating_sub(span)).min(weights[0]);
    Shape { weights, ..land }
}
