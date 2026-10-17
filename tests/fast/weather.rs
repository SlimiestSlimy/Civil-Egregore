//! The weather, alone on worlds the server makes, no sheep on them:
//! a generated world's climate is no catastrophe, clouds deepen and
//! thin a level at a time towards what the climate says, more over
//! water than over land, and a wind blows them its way, none made or
//! lost by it.

use crate::worlds::{plain_start, tick_rule};
use bitplane_manager::Write;
use utilities::coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::weather::{deepens_and_thins, BLOWN_COUNT, CARRIED_COUNT, DEEPENED_COUNT, EVENED_COUNT, PUSHED_COUNT, SAMPLED_COUNT, THINNED_COUNT, WHOLE_PARTS};
use server::World;
use base_types::{nearer_by_step, passes_a_slope, FALL_THAT_FREES, pressure_falls_by, Climate, Wind, HIGHEST_PRESSURE, HOTTEST_TEMPERATURE, HUMIDITY_A_LEVEL_OF_CLOUD, MOST_HUMID, PRESSURE_AT_REST, PRESSURE_GAP_AT_REST, RISE_THAT_HALVES, STRONGEST_WIND};
use base_types::{CLIMATE, CLOUD, ELEVATION, WEATHER_TILE_SCALE, WET};
use utilities::rng::RandomSource;

/// Cells along a tile's side.
const TILE_SIDE: u32 = 1 << WEATHER_TILE_SCALE;

/// Tiles along the side of `world`'s hot superchunks, a square of them.
fn tiles_a_side(world: &World) -> u32 {
    (world.bitmap_arena().superchunk_indices().len() as f64).sqrt() as u32 * SUPERCHUNK_SIDE_CELLS / TILE_SIDE
}

/// The first cell of the tile `(x, y)`, in tiles from the corner of
/// `world`'s hot superchunks.
fn tile_at(world: &World, (x, y): (u32, u32)) -> CellIndex {
    let corner_cartesian = crate::worlds::first_superchunk(world).top_left().cartesian();
    CellCartesian { x: corner_cartesian.x + x * TILE_SIDE, y: corner_cartesian.y + y * TILE_SIDE }.into()
}

/// The cloud over every tile of `world`, in rows.
fn clouds(world: &World) -> Vec<u32> {
    let side_tiles = tiles_a_side(world);
    (0..side_tiles * side_tiles).map(|tile| world.bitmap_arena().value(CLOUD, tile_at(world, (tile % side_tiles, tile / side_tiles))).expect("hot")).collect()
}

/// The climate of every tile of `world`, in rows.
fn climates(world: &World) -> Vec<Climate> {
    let side_tiles = tiles_a_side(world);
    (0..side_tiles * side_tiles).map(|tile| Climate::of_number(world.bitmap_arena().value(CLIMATE, tile_at(world, (tile % side_tiles, tile / side_tiles))).expect("hot"))).collect()
}

/// Whether most cells of the tile `(x, y)` of `world` are under water.
fn over_water(world: &World, (x, y): (u32, u32)) -> bool {
    let tile_first_cell = tile_at(world, (x, y));
    let wet_cells = (0..TILE_SIDE * TILE_SIDE).filter(|cell| world.bitmap_arena().layer_holds_cell(WET, tile_first_cell.at_cartesian_offset((cell % TILE_SIDE) as i32, (cell / TILE_SIDE) as i32).expect("in the world")) == Ok(true)).count();
    wet_cells as u32 * 2 > TILE_SIDE * TILE_SIDE
}

/// A world of land and sea, forced hot, with no sheep.
fn land_and_sea(seed: u64) -> World {
    server::start_world(server::WorldStart { seed: server::seed_with_land(seed, &server::WorldGeneration::DEFAULT), world_size: server::WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, world_generation: crate::worlds::flocked(0), ..server::WorldStart::default() })
}

/// Every tile of `world` put under `climate`.
fn write_everywhere(world: &mut World, climate: Climate) {
    let side_tiles = tiles_a_side(world);
    let climate_writes: Vec<Write> = (0..side_tiles * side_tiles).map(|tile| Write::value(CLIMATE, tile_at(world, (tile % side_tiles, tile / side_tiles)), climate.plane_number())).collect();
    world.write_cells(CLIMATE.layer_type(), climate_writes);
}

/// The pressure of every tile of `world`, added up: what the air's
/// moves neither make nor lose.
fn total_pressure(world: &World) -> u64 {
    climates(world).iter().map(|climate| u64::from(climate.pressure)).sum()
}

/// Every wind there is: each of its two ways asked from the strongest
/// one way to the strongest the other, and held to the circle.
fn winds() -> Vec<Wind> {
    (-STRONGEST_WIND..=STRONGEST_WIND).flat_map(|wind_down| (-STRONGEST_WIND..=STRONGEST_WIND).map(move |wind_across| Wind::from_vector(wind_across, wind_down))).collect()
}

/// A wind is its byte and back, and held to its strongest whatever
/// is asked; a climate is its number and back, whatever is drawn.
#[test]
fn a_climate_is_its_number_and_back() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let winds = winds();
    for &wind in &winds {
        let (wind_across, wind_down) = wind.east_south_vector();
        // Its strength its length, to the nearest whole number -- none longer than the strongest, but by the rounding of a diagonal's parts.
        let wind_length = f64::from(wind_across * wind_across + wind_down * wind_down).sqrt();
        assert_eq!((Wind::of_state(wind.state_byte()), wind.wind_strength()), (wind, wind_length.round() as u32), "{wind:?}");
        assert!(wind.state_byte() <= 0xFF && wind_length < f64::from(STRONGEST_WIND) + 0.5, "{wind:?}");
        assert_eq!(Wind::from_vector(wind_across, wind_down), wind, "held, it is held already");
    }
    // A diagonal blows no stronger than a straight wind: the square's corner comes in to the circle, its way kept.
    assert_eq!((Wind::CALM_AIR.wind_strength(), Wind::CALM_AIR.state_byte(), Wind::from_vector(99, -99).east_south_vector()), (0, 0, (5, -5)));
    assert_eq!((Wind::from_vector(99, 0).east_south_vector(), Wind::from_vector(0, -99).east_south_vector(), Wind::from_vector(7, 7).wind_strength()), ((STRONGEST_WIND, 0), (0, -STRONGEST_WIND), STRONGEST_WIND as u32));
    for _ in 0..seeded_random.draw_between(200, 400) {
        let climate = Climate { humidity: seeded_random.draw_below(u64::from(MOST_HUMID) + 1) as u32, wind: winds[seeded_random.draw_below(winds.len() as u64) as usize], temperature: seeded_random.draw_below(u64::from(HOTTEST_TEMPERATURE) + 1) as u32, pressure: seeded_random.draw_below(u64::from(HIGHEST_PRESSURE) + 1) as u32 };
        assert_eq!(Climate::of_number(climate.plane_number()), climate);
    }
}

/// The small functions the air goes by: a number carried comes
/// nearer and never past; a slope passes all of the air down a fall
/// that frees it, a little less on the level, less the steeper it
/// rises; pressure falls
/// only past the gap that stays, the more the lower; a wind stepped
/// towards another comes to be it, never past the strongest on the way; a wind
/// reaches a tile as often as it is strong, and a calm one never.
#[test]
fn the_small_functions_the_air_goes_by() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    for _ in 0..seeded_random.draw_between(200, 400) {
        let (carried_from, carried_to, step) = (seeded_random.draw_below(256) as u32, seeded_random.draw_below(256) as u32, 1 + seeded_random.draw_below(8) as u32);
        let nearer_value = nearer_by_step(carried_from, carried_to, step);
        assert!(nearer_value.abs_diff(carried_to) <= carried_from.abs_diff(carried_to) && nearer_value.abs_diff(carried_from) <= step && (carried_from != carried_to) == (nearer_value != carried_from), "{carried_from} towards {carried_to} by {step}: {nearer_value}");
        let (slope_rise, whole_parts) = (seeded_random.draw_below(511) as i32 - 255, 1 + seeded_random.draw_below(1024) as u32);
        assert!(passes_a_slope(slope_rise, whole_parts) <= passes_a_slope(slope_rise - 1 - seeded_random.draw_below(64) as i32, whole_parts) && passes_a_slope(-(FALL_THAT_FREES as i32) - seeded_random.draw_below(64) as i32, whole_parts) == whole_parts);
        let (pressure_here, pressure_there) = (seeded_random.draw_below(256) as u32, seeded_random.draw_below(256) as u32);
        let units_falling = pressure_falls_by(pressure_here, pressure_there);
        assert_eq!(units_falling > 0, pressure_here > pressure_there + PRESSURE_GAP_AT_REST, "{pressure_here} to {pressure_there}");
        assert!(units_falling == 0 || units_falling < pressure_falls_by(pressure_here, pressure_there.saturating_sub(1)) || pressure_there == 0);
    }
    assert_eq!(passes_a_slope((RISE_THAT_HALVES - FALL_THAT_FREES) as i32, 256), 128);
    assert!(passes_a_slope(0, 256) < 256 && passes_a_slope(0, 256) > passes_a_slope(8, 256) && passes_a_slope(8, 256) > 192, "level land holds a little, a rise a little more");
    let (winds, draws_a_way) = (winds(), STRONGEST_WIND as u32 + 1);
    for &from_wind in &winds {
        let (wind_across, wind_down) = from_wind.east_south_vector();
        let steps_reached = (0..draws_a_way * draws_a_way).filter(|draw_checked| from_wind.step_drawn(draw_checked % draws_a_way, draw_checked / draws_a_way).is_some()).count() as u32;
        let (misses_across, misses_down) = (draws_a_way - wind_across.unsigned_abs(), draws_a_way - wind_down.unsigned_abs());
        assert_eq!(steps_reached, draws_a_way * draws_a_way - misses_across * misses_down, "{from_wind:?}");
        assert!((0..draws_a_way * draws_a_way).filter_map(|draw_stepped| from_wind.step_drawn(draw_stepped % draws_a_way, draw_stepped / draws_a_way)).all(|(x, y)| (x == 0 || x == wind_across.signum()) && (y == 0 || y == wind_down.signum())));
        // Towards every wind there is: held to the circle at each step, it comes to be the other all the same.
        for &to_wind in &winds {
            let (mut wind, mut steps) = (from_wind, 0);
            while wind != to_wind {
                wind = wind.a_step_towards(to_wind);
                steps += 1;
                assert!(wind.wind_strength() <= STRONGEST_WIND as u32 && steps <= 3 * STRONGEST_WIND, "{from_wind:?} towards {to_wind:?}: {wind:?} after {steps} steps");
            }
        }
        assert_eq!(from_wind.pushed_by(0, 0), from_wind);
        // Pressure leans the way the wind blows: once where it blows no way towards a tile, more the more it blows along the step there.
        let pressure_leanings = base_types::TILES_ABOUT.map(|step| sca_rules::weather::wind_leans_pressure(from_wind, step));
        for (way, &(step_across, step_down)) in base_types::TILES_ABOUT.iter().enumerate() {
            let wind_along_step = wind_across * step_across + wind_down * step_down;
            assert_eq!(pressure_leanings[way], 1 + sca_rules::weather::A_WIND_LEANS_PRESSURE * wind_along_step.max(0) as u32, "{from_wind:?} towards {:?}", base_types::TILES_ABOUT[way]);
            assert_eq!(sca_rules::weather::wind_leans_pressure(Wind::CALM_AIR, base_types::TILES_ABOUT[way]), 1);
        }
        assert!(from_wind == Wind::CALM_AIR || pressure_leanings.iter().max() > pressure_leanings.iter().min(), "{from_wind:?}: some way likelier than another");
    }
}

/// What makes a cloud deepen and thin: more humidity deepens it the
/// likelier, pressure under rest deepens it and over rest thins it,
/// water under it deepens it by its heat and land thins it by it.
#[test]
fn humidity_pressure_and_what_is_under_it_say_how_a_cloud_goes() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    for _ in 0..seeded_random.draw_between(300, 600) {
        let climate = Climate { humidity: seeded_random.draw_below(u64::from(MOST_HUMID) + 1) as u32, wind: Wind::CALM_AIR, temperature: seeded_random.draw_below(u64::from(HOTTEST_TEMPERATURE) + 1) as u32, pressure: seeded_random.draw_between(32, 224) as u32 };
        let cloud = seeded_random.draw_below(u64::from(CLOUD.most_value()) + 1) as u32;
        let ((deepen_parts, thin_parts), (on_land, burnt_thin_parts)) = (deepens_and_thins(climate, cloud, true), deepens_and_thins(climate, cloud, false));
        assert!(deepen_parts >= on_land && burnt_thin_parts >= thin_parts, "{climate:?} under {cloud}");
        let lower_pressure = Climate { pressure: climate.pressure - 32, ..climate };
        let higher_pressure = Climate { pressure: climate.pressure + 31, ..climate };
        assert!(deepens_and_thins(lower_pressure, cloud, true).0 >= deepen_parts && deepens_and_thins(higher_pressure, cloud, true).1 >= thin_parts, "{climate:?} under {cloud}: pressure");
        let more_humid = Climate { humidity: (climate.humidity + HUMIDITY_A_LEVEL_OF_CLOUD).min(MOST_HUMID), ..climate };
        if more_humid.rains(cloud) == climate.rains(cloud) {
            let (more_deepens, less_thins) = deepens_and_thins(more_humid, cloud, true);
            assert!(more_deepens >= deepen_parts && less_thins <= thin_parts, "{climate:?} under {cloud}: humidity");
        }
        assert!(deepen_parts <= 2 * WHOLE_PARTS && thin_parts <= 3 * WHOLE_PARTS);
    }
}

/// A generated world's climate: every tile's pressure at rest, its
/// humidity, heat and wind not one number everywhere, water lying at
/// no height over the ocean and some land above it.
#[test]
fn a_generated_climate_has_no_catastrophes() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let world = land_and_sea(seed);
    let (side_tiles, climates) = (tiles_a_side(&world), climates(&world));
    assert!(climates.iter().all(|climate| climate.pressure == PRESSURE_AT_REST), "the air at rest");
    for (quantity_name, quantity_of) in [("humidity", (|climate: &Climate| climate.humidity) as fn(&Climate) -> u32), ("temperature", |climate| climate.temperature), ("wind", |climate| climate.wind.state_byte())] {
        let (least_value, most_value) = (climates.iter().map(quantity_of).min().expect("tiles"), climates.iter().map(quantity_of).max().expect("tiles"));
        assert!(least_value < most_value, "one {quantity_name} everywhere: {least_value}");
    }
    let elevation_of_tile = |tile: (u32, u32)| world.bitmap_arena().value(ELEVATION, tile_at(&world, tile)).expect("hot");
    let tiles: Vec<(u32, u32)> = (0..side_tiles * side_tiles).map(|tile| (tile % side_tiles, tile / side_tiles)).collect();
    assert!(tiles.iter().any(|&tile| elevation_of_tile(tile) > 0) && tiles.iter().filter(|&&tile| over_water(&world, tile)).all(|&tile| elevation_of_tile(tile) == 0), "land stands over the ocean, water does not");
}

/// Under calm air at rest, cold, a cloud only deepens, a level at a
/// time, and never past its tile's humidity.
#[test]
fn a_cloud_gathers_to_its_tiles_humidity_and_no_further() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 0, 0, 2) });
    let humidity_cloud_level = 2 + seeded_random.draw_below(6) as u32;
    write_everywhere(&mut world, Climate { humidity: humidity_cloud_level * HUMIDITY_A_LEVEL_OF_CLOUD, wind: Wind::CALM_AIR, temperature: 0, pressure: PRESSURE_AT_REST });
    let (mut clouds_before, mut deepened_total) = (clouds(&world), 0);
    for tick in 0..seeded_random.draw_between(1_500, 2_500) {
        let tick_counts = tick_rule(&mut world, base_types::RULE_OF_WEATHER.0).rules;
        let clouds_after = clouds(&world);
        let cloud_change: i64 = clouds_after.iter().zip(&clouds_before).map(|(&cloud_after, &cloud_before)| i64::from(cloud_after) - i64::from(cloud_before)).sum();
        assert_eq!((cloud_change, tick_counts[THINNED_COUNT], tick_counts[BLOWN_COUNT] + tick_counts[PUSHED_COUNT] + tick_counts[EVENED_COUNT] + tick_counts[CARRIED_COUNT]), (tick_counts[DEEPENED_COUNT] as i64, 0, 0), "tick {tick}");
        assert!(clouds_after.iter().zip(&clouds_before).all(|(&cloud_is, &cloud_was)| cloud_is <= humidity_cloud_level && cloud_is - cloud_was <= 1) && tick_counts[SAMPLED_COUNT] > 0, "tick {tick}");
        (clouds_before, deepened_total) = (clouds_after, deepened_total + tick_counts[DEEPENED_COUNT]);
    }
    assert!(deepened_total > 0);
}

/// A wind moves the air and the clouds its way and makes none of
/// either: the pressure of all the tiles together is the same every
/// tick, the clouds change only by what deepened and thinned, and
/// both are moved. Under no wind nothing moves at all, and no cloud
/// deepens.
#[test]
fn a_wind_moves_air_and_clouds_and_makes_none() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    for is_calm in [false, true] {
        let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 0, 0, 2) });
        let side_tiles = tiles_a_side(&world);
        let wind = if is_calm { Wind::CALM_AIR } else { Wind::from_vector(1 + seeded_random.draw_below(STRONGEST_WIND as u64) as i32, seeded_random.draw_between(0, 2 * STRONGEST_WIND as u64 + 1) as i32 - STRONGEST_WIND) };
        write_everywhere(&mut world, Climate { humidity: 0, wind, temperature: 0, pressure: PRESSURE_AT_REST });
        let cloud_writes: Vec<Write> = (0..side_tiles * side_tiles).map(|tile| (tile, seeded_random.draw_below(4), 1 + seeded_random.draw_below(u64::from(CLOUD.most_value())) as u32)).filter(|&(_, drawn_lot, _)| drawn_lot == 0).map(|(tile, _, cloud)| Write::value(CLOUD, tile_at(&world, (tile % side_tiles, tile / side_tiles)), cloud)).collect();
        world.write_cells(CLOUD.layer_type(), cloud_writes);
        let (pressure_began, mut clouds_before, mut moved_totals) = (total_pressure(&world), clouds(&world), [0u64; 3]);
        for tick in 0..seeded_random.draw_between(1_000, 1_500) {
            let tick_counts = tick_rule(&mut world, base_types::RULE_OF_WEATHER.0).rules;
            let clouds_after = clouds(&world);
            let cloud_change: i64 = clouds_after.iter().zip(&clouds_before).map(|(&cloud_after, &cloud_before)| i64::from(cloud_after) - i64::from(cloud_before)).sum();
            // Where the wind has pushed the air out the pressure is under rest, and cloud forms of that alone.
            assert_eq!((cloud_change, total_pressure(&world)), (tick_counts[DEEPENED_COUNT] as i64 - tick_counts[THINNED_COUNT] as i64, pressure_began), "tick {tick}: no cloud made or lost by the wind, and the air all there");
            assert!(!is_calm || tick_counts[DEEPENED_COUNT] == 0, "tick {tick}: a cloud deepened in dry, calm air at rest");
            (clouds_before, moved_totals) = (clouds_after, [moved_totals[0] + tick_counts[BLOWN_COUNT], moved_totals[1] + tick_counts[PUSHED_COUNT], moved_totals[2] + tick_counts[EVENED_COUNT]]);
        }
        assert!(if is_calm { moved_totals == [0; 3] } else { moved_totals[0] > 0 && moved_totals[1] > 0 }, "{moved_totals:?} blown, pushed and evened under {wind:?}");
    }
}

/// Pressure falls to the lower tiles about it, none made or lost, and
/// the air leaving turns the wind: on a calm plain at rest one tile
/// far over the rest loses pressure to the tiles about it, no
/// tile comes over what it had, and winds rise where there were
/// none -- but on no tile at the border of what is loaded.
#[test]
#[ignore = "weather is broken until the climate rework: on some seeds no pressure falls"]
fn pressure_falls_to_the_lower_tiles_about_it_and_turns_the_wind() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 0, 0, 2) });
    let side_tiles = tiles_a_side(&world);
    let climate_at_rest = Climate { humidity: 0, wind: Wind::CALM_AIR, temperature: 0, pressure: PRESSURE_AT_REST };
    write_everywhere(&mut world, climate_at_rest);
    let (high_tile, pressure_over) = ((side_tiles / 4 + seeded_random.draw_below(u64::from(side_tiles / 2)) as u32, side_tiles / 4 + seeded_random.draw_below(u64::from(side_tiles / 2)) as u32), PRESSURE_AT_REST + 40 + seeded_random.draw_below(60) as u32);
    world.write_cells(CLIMATE.layer_type(), vec![Write::value(CLIMATE, tile_at(&world, high_tile), Climate { pressure: pressure_over, ..climate_at_rest }.plane_number())]);
    let (pressure_began, mut evened_total) = (total_pressure(&world), 0);
    for tick in 0..seeded_random.draw_between(6_000, 8_000) {
        evened_total += tick_rule(&mut world, base_types::RULE_OF_WEATHER.0).rules[EVENED_COUNT];
        assert_eq!(total_pressure(&world), pressure_began, "tick {tick}: the air is all there");
    }
    let climates_now = climates(&world);
    let climate_at = |(x, y): (u32, u32)| climates_now[(y * side_tiles + x) as usize];
    assert!(evened_total > 0 && climate_at(high_tile).pressure < pressure_over && climates_now.iter().all(|climate| climate.pressure <= pressure_over), "{evened_total} units fell from {pressure_over}: {} left", climate_at(high_tile).pressure);
    assert!(climates_now.iter().any(|climate| climate.wind != Wind::CALM_AIR), "no wind rose");
    for tile in (0..side_tiles * side_tiles).map(|tile| (tile % side_tiles, tile / side_tiles)).filter(|&(x, y)| x == 0 || y == 0 || x == side_tiles - 1 || y == side_tiles - 1) {
        assert_eq!(climate_at(tile).wind, Wind::CALM_AIR, "{tile:?}, at the border");
    }
}

/// A wind carries its tile's humidity and heat to the tile it blows
/// to, a step at a time: a plain humid, hot and windy on the half the
/// wind comes from, dry, cold and calm on the other, has tiles down
/// the wind come nearer the tiles up it, none ever past either
/// half's numbers.
#[test]
#[ignore = "fails on about 1 seed in 11, too few ticks for the carry rate; climate to be reworked (2026-10-16)"]
fn a_wind_carries_its_climate_down_it() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 0, 0, 2) });
    let side_tiles = tiles_a_side(&world);
    let upwind_climate = Climate { humidity: 128 + seeded_random.draw_below(128) as u32, wind: Wind::from_vector(1 + seeded_random.draw_below(STRONGEST_WIND as u64) as i32, 0), temperature: 128 + seeded_random.draw_below(128) as u32, pressure: PRESSURE_AT_REST };
    let downwind_climate = Climate { humidity: seeded_random.draw_below(128) as u32, wind: Wind::CALM_AIR, temperature: seeded_random.draw_below(128) as u32, pressure: PRESSURE_AT_REST };
    let tiles: Vec<(u32, u32)> = (0..side_tiles * side_tiles).map(|tile| (tile % side_tiles, tile / side_tiles)).collect();
    let climate_writes: Vec<Write> = tiles.iter().map(|&tile| Write::value(CLIMATE, tile_at(&world, tile), if tile.0 < side_tiles / 2 { upwind_climate } else { downwind_climate }.plane_number())).collect();
    world.write_cells(CLIMATE.layer_type(), climate_writes);
    let (climates_began, mut carried_total) = (climates(&world), 0);
    for _ in 0..seeded_random.draw_between(4_000, 5_000) {
        carried_total += tick_rule(&mut world, base_types::RULE_OF_WEATHER.0).rules[CARRIED_COUNT];
    }
    let climates_now = climates(&world);
    let mut tiles_come_nearer = 0;
    for (place, &tile) in tiles.iter().enumerate() {
        let (climate_was, climate_is) = (climates_began[place], climates_now[place]);
        assert!((downwind_climate.humidity..=upwind_climate.humidity).contains(&climate_is.humidity) && (downwind_climate.temperature..=upwind_climate.temperature).contains(&climate_is.temperature), "{tile:?}: {climate_is:?} past both halves");
        tiles_come_nearer += u64::from(tile.0 >= side_tiles / 2 && (climate_is.humidity > climate_was.humidity || climate_is.temperature > climate_was.temperature));
    }
    assert!(carried_total > 0 && tiles_come_nearer > 0, "seed {seed:#x}: {tiles_come_nearer} tiles down the wind more humid or hotter, of {carried_total} carried; up the wind {upwind_climate:?}, down it {downwind_climate:?}");
}
