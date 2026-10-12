//! The weather, alone on worlds the server makes, no sheep on them:
//! a generated world's climate is no catastrophe, clouds deepen and
//! thin a level at a time towards what the climate says, more over
//! water than over land, and a wind blows them its way, none made or
//! lost by it.

use crate::tests::{land_seed, plain_world, tick_rule};
use bitplane_manager::Write;
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::weather::{deepens_and_thins, BLOWN, CARRIED, DEEPENED, EVENED, PUSHED, SAMPLED, THINNED, WHOLE};
use server::World;
use type_registry::{nearer, passes_a_rise, pressure_falls_by, Climate, Wind, CLIMATE, CLOUD, ELEVATION, HIGHEST_PRESSURE, HOTTEST, HUMIDITY_A_LEVEL_OF_CLOUD, MOST_HUMID, PRESSURE_AT_REST, PRESSURE_GAP_AT_REST, RISE_THAT_HALVES, STRONGEST_WIND, WEATHER_TILE_SCALE, WET};
use utilities::rng::Rng;

/// Cells along a tile's side.
const TILE_SIDE: u32 = 1 << WEATHER_TILE_SCALE;

/// Tiles along the side of `world`'s hot superchunks, a square of them.
fn tiles_a_side(world: &World) -> u32 {
    (world.arena().superchunk_indices().len() as f64).sqrt() as u32 * SUPERCHUNK_SIDE_CELLS / TILE_SIDE
}

/// The first cell of the tile `(x, y)`, in tiles from the corner of
/// `world`'s hot superchunks.
fn tile_at(world: &World, (x, y): (u32, u32)) -> CellIndex {
    let corner = crate::tests::first_superchunk(world).top_left().cartesian();
    CellCartesian { x: corner.x + x * TILE_SIDE, y: corner.y + y * TILE_SIDE }.into()
}

/// The cloud over every tile of `world`, in rows.
fn clouds(world: &World) -> Vec<u32> {
    let side = tiles_a_side(world);
    (0..side * side).map(|tile| world.arena().value(CLOUD, tile_at(world, (tile % side, tile / side))).expect("hot")).collect()
}

/// The climate of every tile of `world`, in rows.
fn climates(world: &World) -> Vec<Climate> {
    let side = tiles_a_side(world);
    (0..side * side).map(|tile| Climate::of_number(world.arena().value(CLIMATE, tile_at(world, (tile % side, tile / side))).expect("hot"))).collect()
}

/// Whether most cells of the tile `(x, y)` of `world` are under water.
fn over_water(world: &World, (x, y): (u32, u32)) -> bool {
    let corner = tile_at(world, (x, y));
    let wet = (0..TILE_SIDE * TILE_SIDE).filter(|cell| world.arena().holds(WET, corner.offset((cell % TILE_SIDE) as i32, (cell / TILE_SIDE) as i32).expect("in the world")) == Ok(true)).count();
    wet as u32 * 2 > TILE_SIDE * TILE_SIDE
}

/// A world of land and sea, forced hot, with no sheep.
fn land_and_sea() -> World {
    server::start(server::Start { seed: land_seed(0), size: server::Size::Limited { side: 2, forced: true }, sheep: 0, ..server::Start::default() })
}

/// Every tile of `world` put under `climate`.
fn put_everywhere(world: &mut World, climate: Climate) {
    let side = tiles_a_side(world);
    let everywhere: Vec<Write> = (0..side * side).map(|tile| Write::value(CLIMATE, tile_at(world, (tile % side, tile / side)), climate.number())).collect();
    world.write_cells(CLIMATE.layer_type(), everywhere);
}

/// The pressure of every tile of `world`, added up: what the air's
/// moves neither make nor lose.
fn air(world: &World) -> u64 {
    climates(world).iter().map(|climate| u64::from(climate.pressure)).sum()
}

/// Every wind there is, each of its two ways from the strongest one
/// way to the strongest the other.
fn winds() -> Vec<Wind> {
    (-STRONGEST_WIND..=STRONGEST_WIND).flat_map(|down| (-STRONGEST_WIND..=STRONGEST_WIND).map(move |across| Wind::blowing(across, down))).collect()
}

/// A wind is its byte and back, and held to its strongest whatever
/// is asked; a climate is its number and back, whatever is drawn.
#[test]
fn a_climate_is_its_number_and_back() {
    let mut random = Rng::new(utilities::seed::counted());
    let winds = winds();
    for &wind in &winds {
        let (across, down) = wind.vector();
        assert_eq!((Wind::of_state(wind.state()), wind.strength()), (wind, across.unsigned_abs().max(down.unsigned_abs())));
        assert!(wind.state() <= 0xFF);
    }
    assert_eq!((Wind::CALM.strength(), Wind::CALM.state(), Wind::blowing(99, -99).vector()), (0, 0, (STRONGEST_WIND, -STRONGEST_WIND)));
    for _ in 0..random.between(200, 400) {
        let climate = Climate { humidity: random.below(u64::from(MOST_HUMID) + 1) as u32, wind: winds[random.below(winds.len() as u64) as usize], temperature: random.below(u64::from(HOTTEST) + 1) as u32, pressure: random.below(u64::from(HIGHEST_PRESSURE) + 1) as u32 };
        assert_eq!(Climate::of_number(climate.number()), climate);
    }
}

/// The small functions the air goes by: a number carried comes
/// nearer and never past; a rise passes all of the air on the level,
/// half at the rise that halves, and less the steeper; pressure falls
/// only past the gap that stays, the more the lower; a wind stepped
/// towards another comes to be it, never stronger on the way; a wind
/// reaches a tile as often as it is strong, and a calm one never.
#[test]
fn the_small_functions_the_air_goes_by() {
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..random.between(200, 400) {
        let (from, to, step) = (random.below(256) as u32, random.below(256) as u32, 1 + random.below(8) as u32);
        let is = nearer(from, to, step);
        assert!(is.abs_diff(to) <= from.abs_diff(to) && is.abs_diff(from) <= step && (from != to) == (is != from), "{from} towards {to} by {step}: {is}");
        let (rise, whole) = (random.below(256) as u32, 1 + random.below(1024) as u32);
        assert!(passes_a_rise(rise, whole) <= passes_a_rise(rise / 2, whole) && passes_a_rise(0, whole) == whole);
        let (here, there) = (random.below(256) as u32, random.below(256) as u32);
        let falls = pressure_falls_by(here, there);
        assert_eq!(falls > 0, here > there + PRESSURE_GAP_AT_REST, "{here} to {there}");
        assert!(falls == 0 || falls < pressure_falls_by(here, there.saturating_sub(1)) || there == 0);
    }
    assert_eq!(passes_a_rise(RISE_THAT_HALVES, 256), 128);
    let (winds, draws) = (winds(), STRONGEST_WIND as u32 + 1);
    for &from in &winds {
        let (across, down) = from.vector();
        let reached = (0..draws * draws).filter(|draw| from.step_drawn(draw % draws, draw / draws).is_some()).count() as u32;
        let (misses_across, misses_down) = (draws - across.unsigned_abs(), draws - down.unsigned_abs());
        assert_eq!(reached, draws * draws - misses_across * misses_down, "{from:?}");
        assert!((0..draws * draws).filter_map(|draw| from.step_drawn(draw % draws, draw / draws)).all(|(x, y)| (x == 0 || x == across.signum()) && (y == 0 || y == down.signum())));
        let to = winds[random.below(winds.len() as u64) as usize];
        let (mut wind, mut steps) = (from, 0);
        while wind != to {
            wind = wind.a_step_towards(to);
            steps += 1;
            assert!(wind.strength() <= from.strength().max(to.strength()) && steps <= 2 * STRONGEST_WIND, "{from:?} towards {to:?}: {wind:?} after {steps} steps");
        }
        assert_eq!(from.pushed(0, 0), from);
    }
}

/// What makes a cloud deepen and thin: more humidity deepens it the
/// likelier, pressure under rest deepens it and over rest thins it,
/// water under it deepens it by its heat and land thins it by it.
#[test]
fn humidity_pressure_and_what_is_under_it_say_how_a_cloud_goes() {
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..random.between(300, 600) {
        let climate = Climate { humidity: random.below(u64::from(MOST_HUMID) + 1) as u32, wind: Wind::CALM, temperature: random.below(u64::from(HOTTEST) + 1) as u32, pressure: random.between(32, 224) as u32 };
        let cloud = random.below(u64::from(CLOUD.most()) + 1) as u32;
        let ((deepens, thins), (on_land, burnt)) = (deepens_and_thins(climate, cloud, true), deepens_and_thins(climate, cloud, false));
        assert!(deepens >= on_land && burnt >= thins, "{climate:?} under {cloud}");
        let lower = Climate { pressure: climate.pressure - 32, ..climate };
        let higher = Climate { pressure: climate.pressure + 31, ..climate };
        assert!(deepens_and_thins(lower, cloud, true).0 >= deepens && deepens_and_thins(higher, cloud, true).1 >= thins, "{climate:?} under {cloud}: pressure");
        let more_humid = Climate { humidity: (climate.humidity + HUMIDITY_A_LEVEL_OF_CLOUD).min(MOST_HUMID), ..climate };
        if more_humid.rains(cloud) == climate.rains(cloud) {
            let (more, less) = deepens_and_thins(more_humid, cloud, true);
            assert!(more >= deepens && less <= thins, "{climate:?} under {cloud}: humidity");
        }
        assert!(deepens <= 2 * WHOLE && thins <= 3 * WHOLE);
    }
}

/// A generated world's climate: every tile's pressure at rest, its
/// humidity, heat and wind not one number everywhere, water lying at
/// no height over the ocean and some land above it.
#[test]
fn a_generated_climate_has_no_catastrophes() {
    let world = land_and_sea();
    let (side, climates) = (tiles_a_side(&world), climates(&world));
    assert!(climates.iter().all(|climate| climate.pressure == PRESSURE_AT_REST), "the air at rest");
    for (what, of) in [("humidity", (|climate: &Climate| climate.humidity) as fn(&Climate) -> u32), ("temperature", |climate| climate.temperature), ("wind", |climate| climate.wind.state())] {
        let (least, most) = (climates.iter().map(of).min().expect("tiles"), climates.iter().map(of).max().expect("tiles"));
        assert!(least < most, "one {what} everywhere: {least}");
    }
    let high = |tile: (u32, u32)| world.arena().value(ELEVATION, tile_at(&world, tile)).expect("hot");
    let tiles: Vec<(u32, u32)> = (0..side * side).map(|tile| (tile % side, tile / side)).collect();
    assert!(tiles.iter().any(|&tile| high(tile) > 0) && tiles.iter().filter(|&&tile| over_water(&world, tile)).all(|&tile| high(tile) == 0), "land stands over the ocean, water does not");
}

/// Under calm air at rest, cold, a cloud only deepens, a level at a
/// time, and never past its tile's humidity.
#[test]
fn a_cloud_gathers_to_its_tiles_humidity_and_no_further() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut world = plain_world(4, 0, 0, 2);
    let level = 2 + random.below(6) as u32;
    put_everywhere(&mut world, Climate { humidity: level * HUMIDITY_A_LEVEL_OF_CLOUD, wind: Wind::CALM, temperature: 0, pressure: PRESSURE_AT_REST });
    let (mut before, mut deepened) = (clouds(&world), 0);
    for tick in 0..random.between(1_500, 2_500) {
        let done = tick_rule(&mut world, server::WEATHER_RULE).rules;
        let after = clouds(&world);
        let change: i64 = after.iter().zip(&before).map(|(&is, &was)| i64::from(is) - i64::from(was)).sum();
        assert_eq!((change, done[THINNED], done[BLOWN] + done[PUSHED] + done[EVENED] + done[CARRIED]), (done[DEEPENED] as i64, 0, 0), "tick {tick}");
        assert!(after.iter().zip(&before).all(|(&is, &was)| is <= level && is - was <= 1) && done[SAMPLED] > 0, "tick {tick}");
        (before, deepened) = (after, deepened + done[DEEPENED]);
    }
    assert!(deepened > 0);
}

/// A wind moves the air and the clouds its way and makes none of
/// either: the pressure of all the tiles together is the same every
/// tick, the clouds change only by what deepened and thinned, and
/// both are moved. Under no wind nothing moves at all, and no cloud
/// deepens.
#[test]
fn a_wind_moves_air_and_clouds_and_makes_none() {
    let mut random = Rng::new(utilities::seed::counted());
    for calm in [false, true] {
        let mut world = plain_world(4, 0, 0, 2);
        let side = tiles_a_side(&world);
        let wind = if calm { Wind::CALM } else { Wind::blowing(1 + random.below(STRONGEST_WIND as u64) as i32, random.between(0, 2 * STRONGEST_WIND as u64 + 1) as i32 - STRONGEST_WIND) };
        put_everywhere(&mut world, Climate { humidity: 0, wind, temperature: 0, pressure: PRESSURE_AT_REST });
        let put: Vec<Write> = (0..side * side).map(|tile| (tile, random.below(4), 1 + random.below(u64::from(CLOUD.most())) as u32)).filter(|&(_, lot, _)| lot == 0).map(|(tile, _, cloud)| Write::value(CLOUD, tile_at(&world, (tile % side, tile / side)), cloud)).collect();
        world.write_cells(CLOUD.layer_type(), put);
        let (began, mut before, mut moved) = (air(&world), clouds(&world), [0u64; 3]);
        for tick in 0..random.between(1_000, 1_500) {
            let done = tick_rule(&mut world, server::WEATHER_RULE).rules;
            let after = clouds(&world);
            let change: i64 = after.iter().zip(&before).map(|(&is, &was)| i64::from(is) - i64::from(was)).sum();
            // Where the wind has pushed the air out the pressure is under rest, and cloud forms of that alone.
            assert_eq!((change, air(&world)), (done[DEEPENED] as i64 - done[THINNED] as i64, began), "tick {tick}: no cloud made or lost by the wind, and the air all there");
            assert!(!calm || done[DEEPENED] == 0, "tick {tick}: a cloud deepened in dry, calm air at rest");
            (before, moved) = (after, [moved[0] + done[BLOWN], moved[1] + done[PUSHED], moved[2] + done[EVENED]]);
        }
        assert!(if calm { moved == [0; 3] } else { moved[0] > 0 && moved[1] > 0 }, "{moved:?} blown, pushed and evened under {wind:?}");
    }
}

/// Pressure falls to the lower tiles about it, none made or lost, and
/// the air leaving turns the wind: on a calm plain at rest one tile
/// far over the rest loses pressure to the tiles about it, no
/// tile comes over what it had, and winds rise where there were
/// none -- but on no tile at the border of what is loaded.
#[test]
fn pressure_falls_to_the_lower_tiles_about_it_and_turns_the_wind() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut world = plain_world(4, 0, 0, 2);
    let side = tiles_a_side(&world);
    let rest = Climate { humidity: 0, wind: Wind::CALM, temperature: 0, pressure: PRESSURE_AT_REST };
    put_everywhere(&mut world, rest);
    let (high, over) = ((side / 4 + random.below(u64::from(side / 2)) as u32, side / 4 + random.below(u64::from(side / 2)) as u32), PRESSURE_AT_REST + 40 + random.below(60) as u32);
    world.write_cells(CLIMATE.layer_type(), vec![Write::value(CLIMATE, tile_at(&world, high), Climate { pressure: over, ..rest }.number())]);
    let (began, mut evened) = (air(&world), 0);
    for tick in 0..random.between(6_000, 8_000) {
        evened += tick_rule(&mut world, server::WEATHER_RULE).rules[EVENED];
        assert_eq!(air(&world), began, "tick {tick}: the air is all there");
    }
    let now = climates(&world);
    let at = |(x, y): (u32, u32)| now[(y * side + x) as usize];
    assert!(evened > 0 && at(high).pressure < over && now.iter().all(|climate| climate.pressure <= over), "{evened} units fell from {over}: {} left", at(high).pressure);
    assert!(now.iter().any(|climate| climate.wind != Wind::CALM), "no wind rose");
    for tile in (0..side * side).map(|tile| (tile % side, tile / side)).filter(|&(x, y)| x == 0 || y == 0 || x == side - 1 || y == side - 1) {
        assert_eq!(at(tile).wind, Wind::CALM, "{tile:?}, at the border");
    }
}

/// A wind carries its tile's humidity and heat to the tile it blows
/// to, a step at a time: a plain humid, hot and windy on the half the
/// wind comes from, dry, cold and calm on the other, has tiles down
/// the wind come nearer the tiles up it, none ever past either
/// half's numbers.
#[test]
fn a_wind_carries_its_climate_down_it() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut world = plain_world(4, 0, 0, 2);
    let side = tiles_a_side(&world);
    let upwind = Climate { humidity: 128 + random.below(128) as u32, wind: Wind::blowing(1 + random.below(STRONGEST_WIND as u64) as i32, 0), temperature: 128 + random.below(128) as u32, pressure: PRESSURE_AT_REST };
    let downwind = Climate { humidity: random.below(128) as u32, wind: Wind::CALM, temperature: random.below(128) as u32, pressure: PRESSURE_AT_REST };
    let tiles: Vec<(u32, u32)> = (0..side * side).map(|tile| (tile % side, tile / side)).collect();
    let put: Vec<Write> = tiles.iter().map(|&tile| Write::value(CLIMATE, tile_at(&world, tile), if tile.0 < side / 2 { upwind } else { downwind }.number())).collect();
    world.write_cells(CLIMATE.layer_type(), put);
    let (began, mut carried) = (climates(&world), 0);
    for _ in 0..random.between(4_000, 5_000) {
        carried += tick_rule(&mut world, server::WEATHER_RULE).rules[CARRIED];
    }
    let now = climates(&world);
    let mut nearer = 0;
    for (place, &tile) in tiles.iter().enumerate() {
        let (was, is) = (began[place], now[place]);
        assert!((downwind.humidity..=upwind.humidity).contains(&is.humidity) && (downwind.temperature..=upwind.temperature).contains(&is.temperature), "{tile:?}: {is:?} past both halves");
        nearer += u64::from(tile.0 >= side / 2 && (is.humidity > was.humidity || is.temperature > was.temperature));
    }
    assert!(carried > 0 && nearer > 0, "{nearer} tiles down the wind more humid or hotter, of {carried} carried");
}
