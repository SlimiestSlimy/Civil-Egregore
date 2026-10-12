//! The weather, alone on worlds the server makes, no sheep on them:
//! a generated world's climate is no catastrophe, clouds deepen and
//! thin a level at a time towards what the climate says, more over
//! water than over land, and a wind blows them its way, none made or
//! lost by it.

use crate::tests::{land_seed, plain_world, tick_rule};
use bitplane_manager::{Write, WriteOp};
use coordinates::{CellCartesian, CellIndex, SUPERCHUNK_SIDE_CELLS};
use sca_rules::weather::{deepens_and_thins, BLOWN, DEEPENED, RAINING, SAMPLED, THINNED, WHOLE};
use server::World;
use type_registry::{Climate, Wind, CLIMATE, CLOUD, HOTTEST, MOST_HUMID, STRONGEST_WIND, WAYS_OF_WIND, WEATHER_TILE_SCALE, WET};
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

/// A wind's thirteen states are told apart and read back; the three
/// its four bits have beside are calm; and a climate is its number
/// and back, whatever is drawn.
#[test]
fn a_climate_is_its_number_and_back() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut states = vec![Wind::CALM.state()];
    for (way, strength) in (0..WAYS_OF_WIND.len()).flat_map(|way| (1..=STRONGEST_WIND).map(move |strength| (way, strength))) {
        let wind = Wind::blowing(way, strength);
        assert_eq!((wind.way(), wind.strength(), wind.towards(), Wind::of_state(wind.state())), (Some(way), strength, Some(WAYS_OF_WIND[way]), wind));
        states.push(wind.state());
    }
    states.sort_unstable();
    assert_eq!(states, (0..=12).collect::<Vec<u32>>(), "calm, then four ways at each of three strengths");
    assert!((13..16).all(|state| Wind::of_state(state) == Wind::CALM) && Wind::CALM.towards().is_none() && Wind::CALM.strength() == 0);
    for _ in 0..random.between(200, 400) {
        let climate = Climate { humidity: random.below(u64::from(MOST_HUMID) + 1) as u32, wind: Wind::of_state(random.below(13) as u32), temperature: random.below(u64::from(HOTTEST) + 1) as u32 };
        assert!(climate.number() <= CLIMATE.most());
        assert_eq!(Climate::of_number(climate.number()), climate);
    }
}

/// What a cloud's chances are made of: the more humid the tile, the
/// likelier a level more and the less likely one fewer; the hotter,
/// the likelier a level more over water and one fewer over land; a
/// raining cloud thins the more; and a cloud at its tile's humidity,
/// over cold land, is left as it is.
#[test]
fn humidity_deepens_a_cloud_and_heat_does_what_is_under_it_says() {
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..random.between(500, 1000) {
        let mut draw = |most: u32| random.below(u64::from(most) + 1) as u32;
        let (climate, cloud) = (Climate { humidity: draw(MOST_HUMID - 1), wind: Wind::CALM, temperature: draw(HOTTEST - 1) }, draw(CLOUD.most()));
        let (more_humid, hotter) = (Climate { humidity: climate.humidity + 1, ..climate }, Climate { temperature: (climate.temperature + 8).min(HOTTEST), ..climate });
        for water in [false, true] {
            let ((deepens, thins), (humid_deepens, humid_thins), (hot_deepens, hot_thins)) = (deepens_and_thins(climate, cloud, water), deepens_and_thins(more_humid, cloud, water), deepens_and_thins(hotter, cloud, water));
            // Where the other climate has the same cloud rain and this one not, or the other way, the shedding is what differs: held apart (below).
            if more_humid.rains(cloud) == climate.rains(cloud) {
                assert!(humid_deepens >= deepens && humid_thins <= thins && humid_deepens + thins > deepens + humid_thins, "more humid, {climate:?} under {cloud}");
            }
            if hotter.rains(cloud) != climate.rains(cloud) {
                continue;
            }
            if water {
                assert!(hot_deepens >= deepens && hot_thins == thins, "hotter water, {climate:?} under {cloud}");
            } else {
                assert!(hot_thins >= thins && hot_deepens == deepens, "hotter land, {climate:?} under {cloud}");
            }
        }
        let (water, land) = (deepens_and_thins(Climate { temperature: HOTTEST, ..climate }, cloud, true), deepens_and_thins(Climate { temperature: HOTTEST, ..climate }, cloud, false));
        assert!(water.0 > land.0 && land.1 > water.1, "the hottest: water makes cloud, land burns it off");
        assert!(deepens_and_thins(climate, cloud, false).1 >= u32::from(climate.rains(cloud)) * WHOLE / 8, "a raining cloud sheds");
        let cold = Climate { temperature: 0, ..climate };
        if !cold.rains(cold.humidity) {
            assert_eq!(deepens_and_thins(cold, cold.humidity, false), (0, 0), "at its humidity, cold, not raining");
        }
        // When it rains hangs on both: the hotter, the more cloud the air holds; the more humid, the less -- within what a cloud can be.
        assert!(hotter.rains_above() >= climate.rains_above() && more_humid.rains_above() <= climate.rains_above(), "{climate:?} rains above {}", climate.rains_above());
        assert!(climate.rains_above() < CLOUD.most() && Climate { humidity: 0, temperature: HOTTEST, ..climate }.rains_above() > Climate { humidity: MOST_HUMID, temperature: 0, ..climate }.rains_above());
        assert_eq!(climate.rains(cloud), cloud > climate.rains_above());
    }
}

/// A world generated has a climate over all of it and no catastrophe
/// in it: from a tile to the next, the temperature and the humidity
/// move little where both are land or both water; water is the more
/// humid on the whole.
#[test]
fn a_generated_climate_has_no_catastrophes() {
    let world = land_and_sea();
    let (side, climates) = (tiles_a_side(&world), climates(&world));
    let water: Vec<bool> = (0..side * side).map(|tile| over_water(&world, (tile % side, tile / side))).collect();
    assert!(climates.iter().any(|climate| climate.temperature > 0) && climates.iter().any(|climate| climate.humidity > 0), "a climate generated");
    let mut steps = 0u64;
    for (tile, climate) in climates.iter().enumerate() {
        for next in [tile + 1, tile + side as usize].into_iter().filter(|&next| next < climates.len() && (next % side as usize != 0 || next == tile + side as usize) && water[next] == water[tile]) {
            let other = climates[next];
            assert!(climate.temperature.abs_diff(other.temperature) <= 24, "tile {tile}: {climate:?} beside {other:?}");
            assert!(climate.humidity.abs_diff(other.humidity) <= 2, "tile {tile}: {climate:?} beside {other:?}");
            steps += u64::from(climate.temperature.abs_diff(other.temperature));
        }
    }
    assert!(steps < 4 * climates.len() as u64, "the temperature moves {steps} over {} tiles", climates.len());
    let mean = |wet: bool| {
        let of: Vec<u64> = climates.iter().zip(&water).filter(|(_, is)| **is == wet).map(|(climate, _)| u64::from(climate.humidity)).collect();
        (of.len() > 100).then(|| of.iter().sum::<u64>() as f64 / of.len() as f64)
    };
    if let (Some(wet), Some(dry)) = (mean(true), mean(false)) {
        assert!(wet > dry, "humidity {wet:.2} over water, {dry:.2} over land");
    }
}

/// The weather alone on a plain, half of it put under water, under
/// one climate drawn -- humid, warm, a wind of the least strength:
/// each tick a tile's cloud moves a level at the most, all the clouds
/// together by what deepened less what thinned -- the wind makes none
/// and loses none -- and all of it happens. After a while there is
/// more cloud over the water than over the land.
#[test]
#[allow(clippy::disallowed_methods)]
fn clouds_deepen_and_thin_a_level_at_a_time_and_gather_over_water() {
    let mut world = plain_world(1, 0, 0, 2);
    let mut random = Rng::new(utilities::seed::counted());
    let side = tiles_a_side(&world);
    let climate = Climate { humidity: random.between(5, 11) as u32, wind: Wind::blowing(random.below(WAYS_OF_WIND.len() as u64) as usize, 1), temperature: random.between(96, u64::from(HOTTEST)) as u32 };
    let everywhere: Vec<Write> = (0..side * side).map(|tile| Write::value(CLIMATE, tile_at(&world, (tile % side, tile / side)), climate.number())).collect();
    world.write_cells(CLIMATE.layer_type(), everywhere);
    // The left half under water.
    let corner = tile_at(&world, (0, 0)).cartesian();
    let wet = (0..SUPERCHUNK_SIDE_CELLS).flat_map(|y| (0..SUPERCHUNK_SIDE_CELLS / 2).map(move |x| Write::cell(CellCartesian { x: corner.x + x, y: corner.y + y }.into(), WriteOp::Set)));
    world.write_cells(WET, wet);
    assert!(over_water(&world, (0, 0)) && !over_water(&world, (side - 1, side - 1)), "half water, half land");
    let (mut before, mut all) = (clouds(&world), [0u64; 5]);
    assert!(before.iter().all(|&cloud| cloud == 0), "a world begins with no cloud");
    let ticks = random.between(12_000, 16_000);
    for tick in 0..ticks {
        let done = tick_rule(&mut world, server::WEATHER_RULE).rules;
        let after = clouds(&world);
        let change: i64 = after.iter().zip(&before).map(|(&is, &was)| i64::from(is) - i64::from(was)).sum();
        assert_eq!(change, done[DEEPENED] as i64 - done[THINNED] as i64, "tick {tick}: the clouds together");
        assert!(after.iter().zip(&before).all(|(&is, &was)| is.abs_diff(was) <= 1), "tick {tick}: a cloud moved more than a level");
        let moved = after.iter().zip(&before).filter(|(is, was)| is != was).count() as u64;
        assert_eq!(moved, done[DEEPENED] + done[THINNED] + 2 * done[BLOWN], "tick {tick}: the tiles whose cloud moved");
        [SAMPLED, DEEPENED, THINNED, BLOWN, RAINING].into_iter().for_each(|count| all[count] += done[count]);
        before = after;
    }
    assert!(all[DEEPENED] > 0 && all[THINNED] > 0 && all[BLOWN] > 0, "sampled, deepened, thinned, blown, raining: {all:?}");
    let expected = ticks as f64 * f64::from(side * side) * sca_rules::weather::SAMPLE_CHANCE.fraction();
    assert!((all[SAMPLED] as f64 - expected).abs() < expected / 20.0, "{} tiles sampled, about {expected:.0} expected", all[SAMPLED]);
    // Away from the shore, where the wind brings the other half's cloud.
    let mean = |from: u32| {
        let of: Vec<u64> = (0..side * side).filter(|tile| (from..from + side / 4).contains(&(tile % side))).map(|tile| u64::from(before[tile as usize])).collect();
        of.iter().sum::<u64>() as f64 / of.len() as f64
    };
    let (wet, dry) = (mean(side / 8), mean(side / 2 + side / 8));
    assert!(wet > dry + 1.0, "under {climate:?}: cloud {wet:.2} over water, {dry:.2} over land; counts {all:?}");
}

/// On a plain made dry and cold, where no cloud deepens, under one
/// wind drawn: clouds put in a line across it are found only where
/// the wind blows them to, never behind nor beside; none is made --
/// together they only thin -- and a level blown is one taken and one
/// added. Under no wind, none leaves its tile.
#[test]
fn a_wind_blows_clouds_its_way_and_makes_none() {
    let mut random = Rng::new(utilities::seed::counted());
    for calm in [false, true] {
        let mut world = plain_world(4, 0, 0, 2);
        let side = tiles_a_side(&world);
        let (way, strength) = (random.below(WAYS_OF_WIND.len() as u64) as usize, if calm { 0 } else { 1 + random.below(u64::from(STRONGEST_WIND)) as u32 });
        let climate = Climate { humidity: 0, wind: Wind::blowing(way, strength), temperature: 0 }.number();
        let everywhere: Vec<Write> = (0..side * side).map(|tile| Write::value(CLIMATE, tile_at(&world, (tile % side, tile / side)), climate)).collect();
        world.write_cells(CLIMATE.layer_type(), everywhere);
        assert!(climates(&world).iter().all(|is| is.number() == climate), "one climate everywhere");
        // A line across the middle, athwart the wind: the clouds on it, each of a level drawn.
        let (across, down) = WAYS_OF_WIND[way];
        let on_line = |(x, y): (u32, u32)| if across != 0 { x == side / 2 } else { y == side / 2 };
        let line: Vec<(u32, u32)> = (0..side * side).map(|tile| (tile % side, tile / side)).filter(|&tile| on_line(tile)).collect();
        let put: Vec<Write> = line.iter().map(|&tile| Write::value(CLOUD, tile_at(&world, tile), 1 + random.below(u64::from(CLOUD.most())) as u32)).collect();
        world.write_cells(CLOUD.layer_type(), put);
        let (mut before, mut blown) = (clouds(&world), 0);
        for tick in 0..random.between(1_500, 2_500) {
            let done = tick_rule(&mut world, server::WEATHER_RULE).rules;
            let after = clouds(&world);
            let change: i64 = after.iter().zip(&before).map(|(&is, &was)| i64::from(is) - i64::from(was)).sum();
            assert_eq!((change, done[DEEPENED]), (-(done[THINNED] as i64), 0), "tick {tick}: clouds only thin");
            for (tile, _) in after.iter().enumerate().filter(|(_, cloud)| **cloud > 0) {
                let (x, y) = (tile as i64 % i64::from(side), tile as i64 / i64::from(side));
                // How far down the wind from the line, and how far to its side.
                let (along, middle) = (i64::from(across) * (x - i64::from(side / 2)) + i64::from(down) * (y - i64::from(side / 2)), i64::from(side / 2));
                assert!(along >= 0 && (!calm || along == 0), "tick {tick}: a cloud {along} tiles down a wind of strength {strength}");
                assert!(if across != 0 { x - middle == along * i64::from(across) } else { y - middle == along * i64::from(down) });
            }
            (before, blown) = (after, blown + done[BLOWN]);
        }
        assert_eq!(blown > 0, !calm, "{blown} levels blown by a wind of strength {strength}");
    }
}
