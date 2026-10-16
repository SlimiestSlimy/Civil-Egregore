//! Grass over dirt, on a plain the server makes: it spreads onto dirt at
//! its chance and as the air there is humid, decays by how harsh its
//! tile's climate is and not at all in a mild one, and grows wherever
//! a superchunk is hot.

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, tick_grass};
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use bitplane_manager::Write;
use sca_rules::grass::{DECAYS_COUNT, DECAY_CHANCE, SPREADS_COUNT, SPREAD_CHANCE};
use sca_rules::growing::{climate_harshness, HARSHEST_CLIMATE};
use server::World;
use type_registry::{Climate, Wind, CLIMATE, GRASS, HOTTEST_TEMPERATURE, MOST_HUMID, WEATHER_TILE_SCALE};
use utilities::rng::RandomSource;

/// Every chunk's grass in `world`, as its bitmap's words, the chunks in
/// Morton order.
fn grass_words(world: &World) -> Vec<(ChunkIndex, Vec<u64>)> {
    world.bitmap_arena().buckets_of(GRASS).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect()
}

/// Cells in a superchunk.
const CELLS: u64 = 1 << 20;

/// The first cell of `world`, at its top left.
fn origin(world: &World) -> CellCartesian {
    first_superchunk(world).top_left().cartesian()
}

/// Makes `climate` the climate of every tile of `world`'s first
/// superchunk.
pub fn put_climate(world: &mut World, climate: Climate) {
    let (origin, tile_side) = (origin(world), 1 << WEATHER_TILE_SCALE);
    let tiles = SUPERCHUNK_SIDE_CELLS / tile_side;
    let climate_writes: Vec<Write> = (0..tiles * tiles).map(|tile| Write::value(CLIMATE, CellCartesian { x: origin.x + tile % tiles * tile_side, y: origin.y + tile / tiles * tile_side }.into(), climate.plane_number())).collect();
    world.write_cells(CLIMATE.layer_type(), climate_writes);
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
    let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
    let harsh_climate = Climate { humidity: seeded_random.draw_below(8) as u32, wind: Wind::CALM_AIR, temperature: seeded_random.draw_between(128, u64::from(HOTTEST_TEMPERATURE) + 1) as u32, pressure: type_registry::PRESSURE_AT_REST };
    let mild_climates = [Climate { humidity: MOST_HUMID, ..harsh_climate }, Climate { temperature: 0, ..harsh_climate }];
    for climate in [harsh_climate, mild_climates[seeded_random.draw_below(2) as usize]] {
        let mut world = plain_world(1, 0, 0, 1);
        let origin = origin(&world);
        for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
            plant_grass(&mut world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
        }
        put_climate(&mut world, climate);
        assert_eq!(cells_of_grass(&world), CELLS);
        let (ticks, mut decays_counted) = (seeded_random.draw_between(40, 80), 0);
        for _ in 0..ticks {
            let tick_counts = tick_grass(&mut world).rules;
            assert_eq!(tick_counts[SPREADS_COUNT], 0, "no dirt to spread onto but what decayed, and that is not spread onto in a tick");
            decays_counted += tick_counts[DECAYS_COUNT];
            // What decayed is dirt again, to be spread onto: put back, so every tick is of a superchunk all grass.
            for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|_| tick_counts[DECAYS_COUNT] > 0) {
                plant_grass(&mut world, CellCartesian { x: origin.x + 128 * x, y: origin.y + 128 * y }, 128, 128);
            }
        }
        let expected_decays = ticks as f64 * CELLS as f64 * DECAY_CHANCE.as_fraction() * f64::from(climate_harshness(climate)) / f64::from(HARSHEST_CLIMATE);
        assert!((decays_counted as f64 - expected_decays).abs() <= 4.0 * expected_decays.sqrt(), "under {climate:?}: {decays_counted} decays in {ticks} ticks, about {expected_decays:.0} expected");
        assert_eq!(expected_decays == 0.0, climate_harshness(climate) == 0);
    }
}

/// Over 1,000 ticks the grass changes by
/// no more than what spread and decayed, and scattered grass grows --
/// by no more than spreading alone would make of it.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_changes_by_what_spread_and_decayed() {
    // Grass enough that some of it spreads in so few ticks.
    let mut world = plain_world(1, 100_000, 0, 1);
    let grass_at_start = cells_of_grass(&world);
    let mut grass_before = grass_at_start;
    for _ in 0..1000 {
        let tick_counts = tick_grass(&mut world).rules;
        let grass_now = cells_of_grass(&world);
        assert!(grass_now + tick_counts[DECAYS_COUNT] >= grass_before && grass_now + tick_counts[DECAYS_COUNT] <= grass_before + tick_counts[SPREADS_COUNT], "grown by what spread, less what decayed");
        grass_before = grass_now;
    }
    let grass_growth = grass_before as f64 / grass_at_start as f64;
    assert!(grass_growth > 1.0 && grass_growth < (1.0 + 2.0 * 1000.0 * SPREAD_CHANCE.as_fraction()) * 1.1, "grew {grass_growth:.2} times");
}

/// Grass grows wherever a superchunk is hot: over 5x5 superchunks
/// from the origin, 300 ticks change grass in every one.
#[test]
fn grass_grows_in_every_superchunk() {
    let mut world = plain_world(25, 300_000, 0, 2);
    let superchunks: Vec<SuperchunkIndex> = world.bitmap_arena().superchunk_indices();
        let words_before = grass_words(&world);
    for _ in 0..300 {
        tick_grass(&mut world);
    }
    let words_after = grass_words(&world);
    let mut changed_cells: Vec<CellCartesian> = Vec::new();
    for ((chunk, words_were), (same_chunk, words_are)) in words_before.iter().zip(&words_after) {
        assert_eq!(chunk, same_chunk);
        for (word, (&word_was, &word_is)) in words_were.iter().zip(words_are).enumerate() {
            let flipped_bits = word_was ^ word_is;
            changed_cells.extend((0..64).filter(|bit_flipped| flipped_bits >> bit_flipped & 1 == 1).map(|bit_place| CellIndex::from_chunk_place(*chunk, word * 64 + bit_place).cartesian()));
        }
    }
    assert!(changed_cells.len() > 1000, "grass changed on {} cells", changed_cells.len());
    let superchunk_touched = |superchunk: SuperchunkIndex| changed_cells.iter().any(|&cell| CellIndex::from(cell).superchunk() == superchunk);
    for &superchunk in &superchunks {
        let (x, y) = superchunk.cartesian();
        assert!(superchunk_touched(superchunk), "{x}, {y}: unchanged");
    }
}

/// Grass spreads as often as the air over the cell it would take is
/// humid: scattered the same under three climates, cold so that none
/// decays, it never spreads in the driest, spreads in the most humid,
/// and under a humidity drawn between them spreads less than that --
/// about its share of it, give or take what chance moves so many by.
#[test]
// What is expected is a float; nothing is drawn against it.
#[allow(clippy::disallowed_methods)]
fn grass_spreads_as_the_air_is_humid() {
    let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
    let cold = Climate { humidity: 0, wind: Wind::CALM_AIR, temperature: 0, pressure: type_registry::PRESSURE_AT_REST };
    let humidity_between = seeded_random.draw_between(64, 192) as u32;
    let spreads_and_decays = [0, humidity_between, MOST_HUMID].map(|humidity| {
        let mut world = plain_world(1, 300_000, 0, 1);
        put_climate(&mut world, Climate { humidity, ..cold });
        (0..600).map(|_| tick_grass(&mut world).rules).map(|tick_counts| (tick_counts[SPREADS_COUNT], tick_counts[DECAYS_COUNT])).fold((0, 0), |count_sums, counted_pair| (count_sums.0 + counted_pair.0, count_sums.1 + counted_pair.1))
    });
    assert_eq!((spreads_and_decays[0], spreads_and_decays[1].1, spreads_and_decays[2].1), ((0, 0), 0, 0), "none in the driest, and no decay in the cold");
    let (spreads_between, spreads_most_humid) = (spreads_and_decays[1].0 as f64, spreads_and_decays[2].0 as f64);
    let expected_spreads = spreads_most_humid * f64::from(humidity_between) / f64::from(MOST_HUMID);
    assert!(spreads_most_humid > 100.0 && (spreads_between - expected_spreads).abs() <= 5.0 * spreads_most_humid.sqrt(), "{spreads_between} spreads at humidity {humidity_between}, {spreads_most_humid} at the most: about {expected_spreads:.0} expected");
}
