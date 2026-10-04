//! The lab: in place of the simulation, a thread that only generates
//! -- no sheep, no tick -- to tune by eye how the world is made, its
//! heights ([`shape`]) and where its grass lies ([`world::pasture`]).
//!
//! It answers the window as the simulation does ([`crate::sim`]): the
//! superchunks asked for, wherever in the world the view has gone,
//! each generated the first time it is in view and kept from then on
//! -- the farthest dropped past [`KEPT`] of them. When a slider of generation moves, or the
//! seed is drawn again ([`reseed`]), all it kept is dropped and what
//! is in view is generated afresh.

use crate::sim::{Ask, Cells, Frame, Request, CHUNK_WORDS, SEED};
use crate::tuning::{self, BUMPS, GRASS_COVER, HEIGHT_SPAN, HILLS, PATCH_DETAIL, PATCH_SIZE, RIDGES, ROUGHNESS, SCATTER};
use bitmap::BITS_PER_WORD;
use coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use terrain::Shape;
use utilities::hash::mix;
use world::pasture::{grows, threshold_for, Pasture, ONE};

/// Superchunks kept at most, 128 KiB each: past that, those out of
/// view are dropped, to be generated again if looked at.
const KEPT: usize = 2048;

/// The seed the world is generated from, now.
static SEED_NOW: AtomicU64 = AtomicU64::new(SEED);

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

/// The heights' shape, as the sliders have it: each octave's share of
/// the height span, whole numbers that come to no more than it.
pub fn shape() -> Shape {
    let tuned = tuning::now();
    let shares = [tuned[HILLS], tuned[RIDGES], tuned[BUMPS], tuned[ROUGHNESS]].map(|share| share.max(0.0));
    let all: f32 = shares.iter().sum();
    if all <= 0.0 {
        return Shape { weights: [0; 4] };
    }
    // A height is a byte, whatever is typed.
    let span = tuned[HEIGHT_SPAN].round().clamp(0.0, 255.0) as u64;
    let mut weights = shares.map(|share| (share / all * span as f32).round() as u64);
    // Rounded up together they may pass the span by one: the broadest gives it back.
    weights[0] -= (weights.iter().sum::<u64>().saturating_sub(span)).min(weights[0]);
    Shape { weights }
}

/// How the grass lies, as the sliders have it, in a world of `seed`.
fn pasture(seed: u64) -> Pasture {
    let tuned = tuning::now();
    let of_one = |share: f32| (share.clamp(0.0, 1024.0) * ONE as f32) as u64;
    // What is typed is held to what the noise can take.
    let mut pasture = Pasture { patch: tuned[PATCH_SIZE].round().clamp(0.0, 16.0) as u32, detail: of_one(tuned[PATCH_DETAIL]), scatter: of_one(tuned[SCATTER]), threshold: 0 };
    pasture.threshold = threshold_for(&pasture, seed, of_one(tuned[GRASS_COVER]));
    pasture
}

/// The grass of the superchunk whose top left cell is `top_left`: its
/// chunks' words one after another, as the arena would hold them.
fn grass(pasture: &Pasture, seed: u64, top_left: (u32, u32)) -> Vec<u64> {
    let mut words = vec![0u64; CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        for bit in 0..BITS_PER_WORD {
            let (x, y) = cartesian_from_place(index * BITS_PER_WORD + bit);
            *word |= (grows(pasture, seed, top_left.0.wrapping_add(x), top_left.1.wrapping_add(y)) as u64) << bit;
        }
    }
    words
}

/// Starts the lab on a thread of its own: where to send it requests,
/// and where its frames come back. It stops once the requests' sender
/// is dropped.
pub fn start() -> (Sender<Request>, Receiver<Frame>) {
    let (requests, asked) = channel();
    let (answers, frames) = channel();
    thread::Builder::new()
        .name("lab".to_string())
        .spawn(move || run(&asked, &answers))
        .expect("a thread for the lab");
    (requests, frames)
}

/// The lab's thread: each frame asked for answered, from what is kept
/// and what is generated for it. A change to how the world is
/// generated -- a slider, a new seed -- and nothing is kept: all
/// starts again.
fn run(asked: &Receiver<Request>, answers: &Sender<Frame>) {
    let mut kept: HashMap<SuperchunkIndex, Vec<u64>> = HashMap::new();
    let mut generation = None;
    for request in asked {
        let Request::Sync(ask) = request else {
            // Nothing ticks: there is nothing to pause or to pace.
            continue;
        };
        let asked_at = Instant::now();
        let now = tuning::generation();
        if generation != Some(now) {
            kept.clear();
            generation = Some(now);
        }
        let cells = copy(&mut kept, ask);
        let grass = kept.values().flatten().map(|word| word.count_ones() as u64).sum();
        let frame = Frame { tick: 0, ticks_a_second: 0.0, sheep: 0, grass, sync_seconds: asked_at.elapsed().as_secs_f64(), sync_share: 0.0, detail: ask.detail, near: ask.near, generation: now, cells };
        if answers.send(frame).is_err() {
            return;
        }
    }
}

/// The superchunks `ask` asks for: those not `kept` generated first,
/// each on a thread of its own.
fn copy(kept: &mut HashMap<SuperchunkIndex, Vec<u64>>, ask: Ask) -> Vec<Cells> {
    let asked: Vec<((u32, u32), SuperchunkIndex)> = ask.asked().map(|(x, y)| ((x, y), SuperchunkIndex::from_cartesian(x, y))).collect();
    let top_left = |superchunk: SuperchunkIndex| {
        let CellCartesian { x, y } = superchunk.top_left().cartesian();
        (x, y)
    };
    let missing: Vec<SuperchunkIndex> = asked.iter().map(|&(_, superchunk)| superchunk).filter(|superchunk| !kept.contains_key(superchunk)).collect();
    if !missing.is_empty() {
        if kept.len() + missing.len() > KEPT {
            // Too many kept: those out of view go.
            let (first, last) = (ask.viewport.first, ask.viewport.last);
            kept.retain(|superchunk, _| {
                let (x, y) = superchunk.cartesian();
                (first.0..=last.0).contains(&x) && (first.1..=last.1).contains(&y)
            });
        }
        let seed = seed();
        let pasture = pasture(seed);
        let made: Vec<Vec<u64>> = thread::scope(|scope| {
            let making: Vec<_> = missing.iter().map(|&superchunk| scope.spawn(move || grass(&pasture, seed, top_left(superchunk)))).collect();
            making.into_iter().map(|making| making.join().expect("a superchunk's grass")).collect()
        });
        kept.extend(missing.into_iter().zip(made));
    }
    asked.into_iter().map(|(at, superchunk)| Cells { at, hot: true, top_left: top_left(superchunk), grass: kept[&superchunk].clone(), sheep: Vec::new() }).collect()
}
