//! Grass over dirt, on a plain the server makes: it spreads onto dirt at
//! its chance, decays by how harsh its tile's climate is and not at
//! all in a mild one, and grows wherever a superchunk is hot.

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, tick_grass};
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use bitplane_manager::Write;
use sca_rules::grass::{harshness, DECAYS, DECAY_CHANCE, HARSHEST, SPREADS, SPREAD_CHANCE};
use server::World;
use type_registry::{Climate, Wind, CLIMATE, GRASS, HOTTEST, MOST_HUMID, WEATHER_TILE_SCALE};
use utilities::rng::Rng;

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

/// Makes `climate` the climate of every tile of `world`'s first
/// superchunk.
fn put_climate(world: &mut World, climate: Climate) {
    let (origin, tile_side) = (origin(world), 1 << WEATHER_TILE_SCALE);
    let tiles = SUPERCHUNK_SIDE_CELLS / tile_side;
    let everywhere: Vec<Write> = (0..tiles * tiles).map(|tile| Write::value(CLIMATE, CellCartesian { x: origin.x + tile % tiles * tile_side, y: origin.y + tile / tiles * tile_side }.into(), climate.number())).collect();
    world.write_cells(CLIMATE.layer_type(), everywhere);
}

/// A superchunk all grass, with no dirt to spread onto, under one
/// climate drawn: over some ticks it loses the share of it its chance
/// and the climate's harshness say, give or take four times what
/// chance alone moves that many by -- and none at all where the
/// climate is the most humid, or the coldest, however hot or dry.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_decays_by_how_harsh_its_climate_is() {
    let mut random = Rng::new(utilities::seed::counted());
    let harsh = Climate { humidity: random.below(8) as u32, wind: Wind::CALM, temperature: random.between(128, u64::from(HOTTEST) + 1) as u32 };
    let mild = [Climate { humidity: MOST_HUMID, ..harsh }, Climate { temperature: 0, ..harsh }];
    for climate in [harsh, mild[random.below(2) as usize]] {
        let mut world = plain_world(1, 0, 0, 1);
        let origin = origin(&world);
        for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
            plant_grass(&mut world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
        }
        put_climate(&mut world, climate);
        assert_eq!(cells_of_grass(&world), CELLS);
        let (ticks, mut decays) = (random.between(40, 80), 0);
        for _ in 0..ticks {
            let done = tick_grass(&mut world).rules;
            assert_eq!(done[SPREADS], 0, "no dirt to spread onto but what decayed, and that is not spread onto in a tick");
            decays += done[DECAYS];
            // What decayed is dirt again, to be spread onto: put back, so every tick is of a superchunk all grass.
            for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|_| done[DECAYS] > 0) {
                plant_grass(&mut world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
            }
        }
        let expected = ticks as f64 * CELLS as f64 * DECAY_CHANCE.fraction() * f64::from(harshness(climate)) / f64::from(HARSHEST);
        assert!((decays as f64 - expected).abs() <= 4.0 * expected.sqrt(), "under {climate:?}: {decays} decays in {ticks} ticks, about {expected:.0} expected");
        assert_eq!(expected == 0.0, harshness(climate) == 0);
    }
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
