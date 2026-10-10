//! Grass over dirt, on a plain the server makes: it spreads onto dirt at
//! its chance, decays at its chance times its share of grass
//! neighbours, and grows wherever a superchunk is hot.

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, tick_grass};
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex};
use mc_rules::grass::{DECAYS, DECAY_CHANCE, SAMPLED, SPREADS, SPREAD_CHANCE};
use server::World;
use type_registry::GRASS;

/// Every chunk's grass in `world`, as its bitmap's words, the chunks in
/// Morton order.
fn grass_words(world: &World) -> Vec<(ChunkIndex, Vec<u64>)> {
    world.arena().run(GRASS).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect()
}

/// Cells in a superchunk.
const CELLS: u64 = 1 << 20;

/// The first cell of `world`, at its top left.
fn origin(world: &World) -> CellCartesian {
    first_superchunk(world).top_left().cartesian()
}

/// Grass alone, with no grass around, never decays: a lattice of grass
/// a cell every other each way -- enough of them for a tick to sample
/// some -- for a tick.
#[test]
fn lone_grass_never_decays() {
    let mut world = plain_world(1, 0, 0, 1);
    let origin = origin(&world);
    for (x, y) in (0..512).flat_map(|y| (0..512).map(move |x| (x, y))) {
        plant_grass(&mut world, CellCartesian { x: origin.x + 2 * x, y: origin.y + 2 * y }, 1, 1);
    }
    assert_eq!(cells_of_grass(&world), 512 * 512);
    let done = tick_grass(&mut world).rules;
    assert!(done[SAMPLED] > 0);
    assert_eq!(done[DECAYS], 0);
}

/// Grass with grass all round decays at the whole chance, and has no
/// dirt to spread onto: a superchunk all grass loses about 0.002% of it
/// in a tick -- some twenty cells, give or take three times what chance
/// alone moves that many by; a little less, as cells on its edge see
/// neighbours past it that are not hot.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn surrounded_grass_decays_at_its_chance() {
    let mut world = plain_world(1, 0, 0, 1);
    let origin = origin(&world);
    for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
        plant_grass(&mut world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
    }
    assert_eq!(cells_of_grass(&world), CELLS);
    let done = tick_grass(&mut world).rules;
    let expected = CELLS as f64 * DECAY_CHANCE.fraction();
    assert_eq!(done[SPREADS], 0);
    assert!((done[DECAYS] as f64 - expected).abs() < 3.0 * expected.sqrt(), "{} decays, about {expected:.0} expected", done[DECAYS]);
    assert_eq!(cells_of_grass(&world), CELLS - done[DECAYS]);
}

/// Over 1,000 ticks the grass changes by
/// no more than what spread and decayed, and scattered grass grows --
/// by no more than spreading alone would make of it.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_changes_by_what_spread_and_decayed() {
    let mut world = plain_world(1, 400, 0, 1);
    let start = cells_of_grass(&world);
    let mut grass = start;
    for _ in 0..1000 {
        let done = tick_grass(&mut world).rules;
        let now = cells_of_grass(&world);
        assert!(now + done[DECAYS] >= grass && now + done[DECAYS] <= grass + done[SPREADS], "grown by what spread, less what decayed");
        grass = now;
    }
    let growth = grass as f64 / start as f64;
    assert!(growth > 1.0 && growth < (1.0 + 2.0 * 1000.0 * SPREAD_CHANCE.fraction()) * 1.1, "grew {growth:.2} times");
}

/// Grass grows wherever a superchunk is hot: over 5x5 superchunks
/// from the origin, 300 ticks change grass in every one.
#[test]
fn grass_grows_in_every_superchunk() {
    let mut world = plain_world(25, 300_000, 0, 2);
    let superchunks: Vec<SuperchunkIndex> = world.arena().superchunk_indices();
        let before = grass_words(&world);
    for _ in 0..300 {
        tick_grass(&mut world);
    }
    let after = grass_words(&world);
    let mut changed: Vec<CellCartesian> = Vec::new();
    for ((chunk, was), (same, is)) in before.iter().zip(&after) {
        assert_eq!(chunk, same);
        for (word, (&was, &is)) in was.iter().zip(is).enumerate() {
            let flipped = was ^ is;
            changed.extend((0..64).filter(|bit| flipped >> bit & 1 == 1).map(|bit| CellIndex::of(*chunk, word * 64 + bit).cartesian()));
        }
    }
    assert!(changed.len() > 1000, "grass changed on {} cells", changed.len());
    let touched = |superchunk: SuperchunkIndex| changed.iter().any(|&cell| CellIndex::from(cell).superchunk() == superchunk);
    for &superchunk in &superchunks {
        let (x, y) = superchunk.cartesian();
        assert!(touched(superchunk), "{x}, {y}: unchanged");
    }
}
