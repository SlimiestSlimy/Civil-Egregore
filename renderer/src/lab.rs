//! The lab: the renderer run to tune by eye how the world is made and
//! how it changes -- a world of the superchunks shown, every one hot
//! from the start, no sheep, its rules ticking ([`crate::sim`]). What
//! is here is what the sliders say of generation: the seed, the
//! heights' shape, how the grass and the trees lie. When a slider of
//! generation moves, or the seed is drawn again ([`reseed`]), the
//! world is made afresh and its ticks start from 0.

use crate::sim;
use crate::tuning::{self, CLUMPING, COAST_BREADTH, COAST_LOWNESS, FINER_DEPTH, FINER_FALL, FINER_HEIGHT, FINER_SHARE, GRASS_COVER, GRASS_DETAIL, GRASS_PATCH, GRASS_SCATTER, HIGHEST_LAND, LEAST_SIGMOID, LINE_BENDING, MOST_SIGMOID, NARROWEST_BLEND, OCEAN_FLOOR, OCEAN_LEVEL, OCEAN_SHARE, WEIGHT_SPREAD, RAISED_SHARE, TREE_COVER, TREE_DETAIL, TREE_PATCH, TREE_SCATTER, VERTEX_SPACING, WIDEST_BLEND};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use worldgen::mesh::SIGMOID_ONE;
use worldgen::Shape;
use utilities::hash::mix;
use worldgen::patches::Patches;
use worldgen::ONE;
use world::Generation;

/// Whether the lab is what runs.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// The seed drawn last, 0 while none was: the world is then generated
/// from the run's own ([`sim::seed`]).
static SEED_DRAWN: AtomicU64 = AtomicU64::new(0);

/// Says the lab is what runs: generation is then the sliders'.
pub fn run() {
    RUNNING.store(true, Ordering::Relaxed);
}

/// Whether the seed was drawn again and the view has not gone back to
/// where it started since.
static VIEW_RESET: AtomicBool = AtomicBool::new(false);

/// Whether the view is to go back to where it started: once for each
/// time the seed is drawn.
pub fn view_reset() -> bool {
    VIEW_RESET.swap(false, Ordering::Relaxed)
}

/// The seed the world is generated from, now.
pub fn seed() -> u64 {
    match SEED_DRAWN.load(Ordering::Relaxed) {
        0 => sim::seed(),
        drawn => drawn,
    }
}

/// Draws a new seed, off the clock: the world is generated again, and
/// the view goes back to where it started.
pub fn reseed() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos() as u64);
    SEED_DRAWN.store(mix(now).max(1), Ordering::Relaxed);
    tuning::regenerate();
    VIEW_RESET.store(true, Ordering::Relaxed);
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
    let shape = shape(&tuned);
    Generation { shape, grass: patches([GRASS_COVER, GRASS_PATCH, GRASS_DETAIL, GRASS_SCATTER]), trees: patches([TREE_COVER, TREE_PATCH, TREE_DETAIL, TREE_SCATTER]) }
}

/// The heights' shape, as the sliders have it: what is typed held to
/// what a height and the mesh can take.
fn shape(tuned: &tuning::Tuning) -> Shape {
    let height = |tuned: f32| tuned.round().clamp(0.0, u16::MAX as f32) as u16;
    let share = |tuned: f32| (tuned.clamp(0.0, 1.0) * ONE as f32) as u64;
    let sigmoid = |tuned: f32| (tuned.clamp(1.0, 16.0) * SIGMOID_ONE as f32) as u64;
    let ground = height(tuned[OCEAN_FLOOR]);
    Shape {
        ground,
        // No ocean under its own floor.
        ocean: height(tuned[OCEAN_LEVEL]).max(ground),
        span: tuned[VERTEX_SPACING].round().clamp(6.0, 24.0) as u32,
        sea: share(tuned[OCEAN_SHARE]),
        highest: height(tuned[HIGHEST_LAND]),
        coast: tuned[COAST_BREADTH].round().clamp(0.0, 4.0) as u32,
        coast_low: sigmoid(tuned[COAST_LOWNESS]),
        clumping: share(tuned[CLUMPING]),
        narrow: share(tuned[NARROWEST_BLEND]),
        wide: share(tuned[WIDEST_BLEND]),
        soft: sigmoid(tuned[LEAST_SIGMOID]),
        hard: sigmoid(tuned[MOST_SIGMOID]),
        warp: (tuned[LINE_BENDING].clamp(0.0, 4.0) * ONE as f32) as u64,
        finer_depth: tuned[FINER_DEPTH].round().clamp(0.0, 10.0) as u32,
        finer_fall: share(tuned[FINER_FALL]),
        weight: share(tuned[WEIGHT_SPREAD]),
        finer_share: share(tuned[FINER_SHARE]),
        finer_height: height(tuned[FINER_HEIGHT]) as u64,
        raised: share(tuned[RAISED_SHARE]),
    }
}
