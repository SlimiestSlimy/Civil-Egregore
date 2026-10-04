//! The lab: the renderer run to tune by eye how the world is made and
//! how it changes -- a world of the superchunks shown, every one hot
//! from the start, no sheep, its rules ticking ([`crate::sim`]). What
//! is here is what the sliders say of generation: the seed, the
//! heights' shape, how the grass and the trees lie. When a slider of
//! generation moves, or the seed is drawn again ([`reseed`]), the
//! world is made afresh and its ticks start from 0.

use crate::sim::SEED;
use crate::tuning::{self, COAST, GRASS_COVER, GROUND_LEVEL, HEIGHT_SPAN, HILLS, HILL_SINK, LAND_RISE, LAND_SPAN, OCEAN_DEPTH, OCEAN_SHARE, PATCH_DETAIL, PATCH_SIZE, RISE_SHARES, SCATTER, SHORE_SPAN, SHORE_WANDER, TREE_COVER, TREE_DETAIL, TREE_PATCH, TREE_SCATTER};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use terrain::Shape;
use utilities::hash::{mix, GOLDEN_RATIO};
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
    SEED_NOW.load(Ordering::Relaxed)
}

/// Draws a new seed, off the clock: the world is generated again, and
/// the view goes back to where it started.
pub fn reseed() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos() as u64);
    SEED_NOW.store(mix(now), Ordering::Relaxed);
    tuning::regenerate();
    VIEW_RESET.store(true, Ordering::Relaxed);
}

/// How the world is generated now: as the sliders have it in the lab,
/// as worlds are otherwise.
pub fn generation() -> Generation {
    if !RUNNING.load(Ordering::Relaxed) {
        return Generation::DEFAULT;
    }
    // Worked out once for each change: the ocean's level takes thousands of cells to find.
    let key = (tuning::generation(), seed());
    let mut last = LAST.lock().expect("the last generation");
    if let Some((_, generation)) = last.filter(|&(made_for, _)| made_for == key) {
        return generation;
    }
    let generation = from_sliders();
    *last = Some((key, generation));
    generation
}

/// The last generation worked out from the sliders, and the count of
/// changes and the seed it was worked out for.
static LAST: Mutex<Option<((u64, u64), Generation)>> = Mutex::new(None);

/// Cells looked at to find the ocean's level.
const SAMPLED: u64 = 1 << 14;

/// The height `share` of the land of `shape` is under, in the world of
/// `seed`: found from [`SAMPLED`] cells drawn over the world.
fn ocean_level(shape: &Shape, seed: u64, share: f32) -> u16 {
    let mut lands: Vec<u64> = (0..SAMPLED).map(|sample| mix(sample.wrapping_mul(GOLDEN_RATIO))).map(|drawn| shape.ground as u64 + terrain::rise(shape, seed, (drawn >> 32) as u32, drawn as u32)).collect();
    lands.sort_unstable();
    match (share.clamp(0.0, 1.0) * SAMPLED as f32) as usize {
        0 => 0,
        under if under >= lands.len() => u16::MAX,
        under => lands[under].min(u16::MAX as u64) as u16,
    }
}

/// How the world is generated, as the sliders have it.
fn from_sliders() -> Generation {
    let tuned = tuning::now();
    // What is typed is held to what the noise can take.
    let of_one = |share: f32| (share.clamp(0.0, 1024.0) * ONE as f32) as u64;
    let patches = |[cover, patch, detail, scatter]: [usize; 4]| Patches { cover: of_one(tuned[cover]), patch: tuned[patch].round().clamp(0.0, 16.0) as u32, detail: of_one(tuned[detail]), scatter: of_one(tuned[scatter]) };
    let land = shape(&tuned);
    let shape = Shape { ocean: ocean_level(&land, seed(), tuned[OCEAN_SHARE]), ..land };
    Generation { shape, grass: patches([GRASS_COVER, PATCH_SIZE, PATCH_DETAIL, SCATTER]), trees: patches([TREE_COVER, TREE_PATCH, TREE_DETAIL, TREE_SCATTER]) }
}

/// The heights' shape, as the sliders have it, but for the ocean's
/// level: the land,
/// and each hill octave's share of the height span, whole numbers that
/// come to no more than the span.
fn shape(tuned: &tuning::Tuning) -> Shape {
    // The hills are a height high at most, whatever is typed.
    let span = tuned[HEIGHT_SPAN].round().clamp(0.0, u16::MAX as f32) as u64;
    // A height is 16 bits, whatever is typed: what the land and its hills would pass is held to the highest.
    let height = |tuned: f32| tuned.round().clamp(0.0, u16::MAX as f32) as u16;
    let land = Shape {
        weights: [0; 11],
        ocean: 0,
        sunk: (tuned[HILL_SINK].clamp(0.0, 1.0) * ONE as f32) as u64,
        depth: tuned[OCEAN_DEPTH].round().clamp(0.0, u16::MAX as f32) as u64,
        coast: tuned[COAST].round().clamp(1.0, u16::MAX as f32) as u64,
        ground: height(tuned[GROUND_LEVEL]),
        rise: height(tuned[LAND_RISE]) as u64,
        rise_span: tuned[LAND_SPAN].round().clamp(5.0, 24.0) as u32,
        rise_shares: std::array::from_fn(|octave| (tuned[RISE_SHARES + octave].clamp(0.0, 16.0) * 1024.0) as u64),
        shore: (tuned[SHORE_WANDER].clamp(0.0, 16.0) * ONE as f32) as u64,
        shore_span: tuned[SHORE_SPAN].round().clamp(2.0, 24.0) as u32,
    };
    let shares: [f32; 11] = std::array::from_fn(|octave| tuned[HILLS + octave].max(0.0));
    let all: f32 = shares.iter().sum();
    if all <= 0.0 {
        return land;
    }
    let mut weights = shares.map(|share| (share / all * span as f32).round() as u64);
    // Rounded up together they may pass the span by a few: the largest gives them back.
    let most = (0..weights.len()).max_by_key(|&octave| weights[octave]).unwrap_or(0);
    weights[most] -= (weights.iter().sum::<u64>().saturating_sub(span)).min(weights[most]);
    Shape { weights, ..land }
}
