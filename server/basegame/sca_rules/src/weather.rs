//! The weather: clouds over the climate, a tile 8 by 8 cells at a
//! time. A tile sampled has its cloud blown a step by its wind, or
//! deepened or thinned by its humidity, its heat and what is under it
//! (`docs/sca_rules.md`, "Weather").

use simulation::simulation_api::simulation_time::SimulationTime;
use base_types::{passes_a_slope, pressure_falls_by, Climate, CLIMATE, CLOUD, ELEVATION, HIGHEST_PRESSURE, HUMIDITY_A_LEVEL_OF_CLOUD, PRESSURE_AT_REST, STRONGEST_WIND, TILES_ABOUT, WEATHER_TILE_SCALE, WET, Wind};
use simulation::simulation_api::{on_each_sampled, RuleCode, cell_reads_and_writes, place_counted, tick_and_random_stream, CellIndex, Chance, RuleCounts, SimulationTickState};

/// The simulation time from one sample of a tile to the next, on
/// average: what its chance each tick is worked out from, so the
/// weather goes as fast at any tick rate.
pub const MEAN_TIME_TO_SAMPLE: SimulationTime = SimulationTime::from_seconds(13);
/// The chance, each tick, that a tile is sampled.
pub const SAMPLE_CHANCE: Chance = MEAN_TIME_TO_SAMPLE.chance_a_tick();
/// What a sample's chances are parts of: one number drawn under it
/// says what the tile does.
pub const WHOLE_PARTS: u32 = 256;
/// The parts with which a cloud deepens for each level it is under
/// its tile's humidity, and thins for each level over it.
pub const PARTS_A_LEVEL_FROM_HUMIDITY: u32 = WHOLE_PARTS / HUMIDITY_A_LEVEL_OF_CLOUD;
/// Over water the temperature, divided by this, is parts with which
/// the cloud deepens: what the heat takes up of the water.
pub const EVAPORATION_DIVIDES_HEAT: u32 = 16;
/// Over land the temperature, divided by this, is parts with which
/// the cloud thins: what the heat burns off.
pub const BURNING_OFF_DIVIDES_HEAT: u32 = 4;
/// The parts with which a raining cloud thins besides.
pub const PARTS_RAIN_SHEDS: u32 = WHOLE_PARTS / 4;
/// How far a tile's pressure is from rest, divided by this, is parts
/// with which its cloud deepens -- under rest, air rising -- or
/// thins -- over it, air sinking.
pub const PRESSURE_DIVIDES_ITS_GAP: u32 = 2;
/// Cells along a side of a tile of the weather.
const TILE_SIDE: i32 = 1 << WEATHER_TILE_SCALE;

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNT_NAMES: [&str; 8] = ["sampled", "deepened", "thinned", "blown", "raining", "carried", "pushed", "evened"];
/// What runs the weather's rule, as it is registered: [`rule`] on each
/// sample of its master, chosen with [`SAMPLE_CHANCE`].
pub const CODE: RuleCode = RuleCode { count_names: &COUNT_NAMES, run: |tick_state, context| on_each_sampled(tick_state, context.master, SAMPLE_CHANCE, rule) };
/// Tiles sampled.
pub const SAMPLED_COUNT: usize = place_counted(&COUNT_NAMES, "sampled");
/// Levels of cloud added: counted as each is applied.
pub const DEEPENED_COUNT: usize = place_counted(&COUNT_NAMES, "deepened");
/// Levels of cloud taken.
pub const THINNED_COUNT: usize = place_counted(&COUNT_NAMES, "thinned");
/// Levels of cloud a wind moved to the tile it blew to.
pub const BLOWN_COUNT: usize = place_counted(&COUNT_NAMES, "blown");
/// Tiles sampled that were raining.
pub const RAINING_COUNT: usize = place_counted(&COUNT_NAMES, "raining");
/// Tiles a wind carried humidity, heat or itself onto.
pub const CARRIED_COUNT: usize = place_counted(&COUNT_NAMES, "carried");
/// Units of pressure a wind pushed onto the tile it blew to.
pub const PUSHED_COUNT: usize = place_counted(&COUNT_NAMES, "pushed");
/// Units of pressure that fell from a tile to a lower one beside it.
pub const EVENED_COUNT: usize = place_counted(&COUNT_NAMES, "evened");

/// How many times likelier, for each unit of a tile's wind along the
/// step to a tile, its pressure falls to that one than to one the
/// wind does not blow to at all ([`wind_leans_pressure`]).
pub const A_WIND_LEANS_PRESSURE: u32 = 4;
/// The simulation time a wind blowing takes to carry something of its
/// tile's climate to the next, on average, were it to reach it every
/// sample: slowly, beside the air and the clouds it moves.
pub const MEAN_TIME_TO_CARRY_CLIMATE: SimulationTime = SimulationTime::from_seconds(400);
/// The chance, each sample its wind reaches a tile in, that it
/// carries climate and not air.
pub const CARRY_CHANCE: Chance = MEAN_TIME_TO_CARRY_CLIMATE.chance_each(MEAN_TIME_TO_SAMPLE);
/// What a wind carries of a climate, one of them drawn each time:
/// its humidity, its temperature, itself.
const CARRIED_OF_A_CLIMATE: u64 = 3;

/// The tile a step of `(tiles_across, tiles_down)` tiles from `tile`, and its
/// climate: none past the world's edge or where it is not loaded.
fn tile_beside(tick_state: &SimulationTickState, tile: CellIndex, (tiles_across, tiles_down): (i32, i32)) -> Option<(CellIndex, Climate)> {
    let beside_tile = tile.at_cartesian_offset(tiles_across * TILE_SIDE, tiles_down * TILE_SIDE)?;
    cell_reads_and_writes::value(tick_state, CLIMATE, beside_tile).map(|climate| (beside_tile, Climate::of_number(climate)))
}

/// Whether the tile at `tile` has a tile about it that is not
/// loaded, or is past the world's edge: its wind is what comes in
/// from there, and nothing inside turns it.
fn at_the_border_of_what_is_loaded(tick_state: &SimulationTickState, tile: CellIndex) -> bool {
    TILES_ABOUT.iter().any(|&step| tile_beside(tick_state, tile, step).is_none())
}

/// The parts of `whole_parts` with which air goes from the tile at `from_tile`
/// to the one at `to_tile`: the more downhill, the less uphill, mildly
/// either way (`passes_a_slope`) -- the one resistance of the land,
/// to pressure and to wind alike.
fn air_passes(tick_state: &SimulationTickState, from_tile: CellIndex, to_tile: CellIndex, whole_parts: u32) -> u32 {
    let elevation_of = |tile| cell_reads_and_writes::value(tick_state, ELEVATION, tile).unwrap_or(0) as i32;
    passes_a_slope(elevation_of(to_tile) - elevation_of(from_tile), whole_parts)
}

/// A unit of pressure goes from the tile at `from_tile`, whose climate
/// `climate_here` becomes `climate_left` but for that unit, to the one at `to_tile`, of
/// climate `climate_there`: both climates under both compares, so no air is
/// made or lost wherever a border falls between them.
fn give_pressure(tick_state: &mut SimulationTickState, (from_tile, climate_here, climate_left): (CellIndex, Climate, Climate), (to_tile, climate_there): (CellIndex, Climate), count_place: usize) {
    let (from_climate_after, to_climate_after) = (Climate { pressure: climate_here.pressure - 1, ..climate_left }, Climate { pressure: climate_there.pressure + 1, ..climate_there });
    cell_reads_and_writes::change_both(tick_state, CLIMATE, (from_tile, climate_here.plane_number(), from_climate_after.plane_number()), (to_tile, climate_there.plane_number(), to_climate_after.plane_number()), count_place);
}

/// How much likelier pressure falls to the tile the step `step_tiles`
/// away for the wind `wind` over the tile it leaves: once with no
/// wind that way, and [`A_WIND_LEANS_PRESSURE`] times more for each
/// unit of the wind along the step -- so air goes with the wind it
/// made, past where the pressures would have evened, and comes back:
/// what gives the weather its spring.
pub const fn wind_leans_pressure(wind: Wind, step_tiles: (i32, i32)) -> u32 {
    let (wind_across, wind_down) = wind.east_south_vector();
    let wind_along_step = wind_across * step_tiles.0 + wind_down * step_tiles.1;
    1 + A_WIND_LEANS_PRESSURE * if wind_along_step > 0 { wind_along_step as u32 } else { 0 }
}

/// Pressure falls: the tile at `sampled_tile`, of climate `climate_here`, gives a
/// unit of its pressure to one of the tiles about it far enough
/// under it (`pressure_falls_by`), drawn among them by how far under
/// each is, how much its own wind blows that way ([`wind_leans_pressure`]) and
/// how little the land rises to it ([`air_passes`]) -- the lowest and
/// the most downwind the likeliest, none higher ever, and up a slope
/// less often. The air leaving turns the tile's own wind that way
/// (`Wind::pushed_by`), unless it is at the border of what is loaded.
/// Whether a unit went.
fn pressure_falls(tick_state: &mut SimulationTickState, sampled_tile: CellIndex, climate_here: Climate) -> bool {
    let tiles_beside = TILES_ABOUT.map(|step| tile_beside(tick_state, sampled_tile, step));
    let mut fall_weights = tiles_beside.map(|tile_and_climate| tile_and_climate.map_or(0, |(_, climate_beside)| pressure_falls_by(climate_here.pressure, climate_beside.pressure)));
    for (way, fall_weight) in fall_weights.iter_mut().enumerate() {
        *fall_weight *= wind_leans_pressure(climate_here.wind, TILES_ABOUT[way]);
    }
    let fall_weight_sum: u32 = fall_weights.iter().sum();
    if fall_weight_sum == 0 || climate_here.pressure == 0 {
        return false;
    }
    // One number drawn over all that would fall on level land: what the land holds back of each is where nothing goes.
    let mut drawn_part = tick_and_random_stream::random_stream(tick_state).draw_below(u64::from(fall_weight_sum * WHOLE_PARTS)) as u32;
    for (way, &way_fall_weight) in fall_weights.iter().enumerate().filter(|(_, fall_weight_filtered)| **fall_weight_filtered > 0) {
        let Some((towards_tile, climate_there)) = tiles_beside[way] else {
            continue;
        };
        if drawn_part < way_fall_weight * air_passes(tick_state, sampled_tile, towards_tile, WHOLE_PARTS) {
            let wind = if tiles_beside.iter().all(Option::is_some) { climate_here.wind.pushed_by(TILES_ABOUT[way].0, TILES_ABOUT[way].1) } else { climate_here.wind };
            give_pressure(tick_state, (sampled_tile, climate_here, Climate { wind, ..climate_here }), (towards_tile, climate_there), EVENED_COUNT);
            return true;
        }
        drawn_part = drawn_part.saturating_sub(way_fall_weight * WHOLE_PARTS);
    }
    false
}

/// The wind of the tile of climate `climate_here` carries something of it
/// onto the tile at `towards_tile`, of climate `climate_there`: its humidity, its
/// temperature or its own vector, one drawn, each a step nearer the
/// tile it comes from (`Climate::with_humidity_carried_from` and the
/// two beside it). A tile at the border of what is loaded keeps its
/// wind. Nothing is queued where the two are alike already.
fn carry_climate(tick_state: &mut SimulationTickState, climate_here: Climate, towards_tile: CellIndex, climate_there: Climate) {
    let climate_carried = match tick_and_random_stream::random_stream(tick_state).draw_below(CARRIED_OF_A_CLIMATE) {
        0 => climate_there.with_humidity_carried_from(climate_here),
        1 => climate_there.with_temperature_carried_from(climate_here),
        _ if at_the_border_of_what_is_loaded(tick_state, towards_tile) => climate_there,
        _ => climate_there.with_wind_carried_from(climate_here),
    };
    if climate_carried != climate_there {
        cell_reads_and_writes::set_value_counted(tick_state, CLIMATE, towards_tile, climate_there.plane_number(), climate_carried.plane_number(), CARRIED_COUNT);
    }
}

/// The wind blows: that of the tile at `sampled_tile`, of climate `climate_here`
/// under `cloud`, reaches one of the eight tiles about it -- drawn by
/// its vector, the stronger the likelier (`Wind::step_drawn`) -- if
/// the land lets it ([`air_passes`]). There it carries climate, now and
/// then ([`carry_climate`]), or else pushes a unit of its tile's pressure
/// onto that tile -- so winds that meet on a tile raise it -- and
/// with it a level of its cloud, if it has one and there is room.
/// Whether a level of cloud went.
fn wind_blows(tick_state: &mut SimulationTickState, sampled_tile: CellIndex, climate_here: Climate, cloud: u32) -> bool {
    let wind_draw_bound = STRONGEST_WIND as u64 + 1;
    let (drawn_across, drawn_down) = (tick_and_random_stream::random_stream(tick_state).draw_below(wind_draw_bound) as u32, tick_and_random_stream::random_stream(tick_state).draw_below(wind_draw_bound) as u32);
    let Some((towards_tile, climate_there)) = climate_here.wind.step_drawn(drawn_across, drawn_down).and_then(|step| tile_beside(tick_state, sampled_tile, step)) else {
        return false;
    };
    if tick_and_random_stream::random_stream(tick_state).draw_below(u64::from(WHOLE_PARTS)) as u32 >= air_passes(tick_state, sampled_tile, towards_tile, WHOLE_PARTS) {
        return false;
    }
    if tick_and_random_stream::random_stream(tick_state).chance(CARRY_CHANCE) {
        carry_climate(tick_state, climate_here, towards_tile, climate_there);
        return false;
    }
    if climate_here.pressure > 0 && climate_there.pressure < HIGHEST_PRESSURE {
        give_pressure(tick_state, (sampled_tile, climate_here, climate_here), (towards_tile, climate_there), PUSHED_COUNT);
    }
    if cloud > 0
        && let Some(over_there) = cell_reads_and_writes::value(tick_state, CLOUD, towards_tile).filter(|&over_there| over_there < CLOUD.most_value())
    {
        cell_reads_and_writes::move_one(tick_state, CLOUD, sampled_tile, cloud, towards_tile, over_there, BLOWN_COUNT);
        return true;
    }
    false
}

/// The parts of [`WHOLE_PARTS`] with which the cloud at level `cloud` over
/// a tile of `climate` deepens a level, and with which it thins one:
/// towards the tile's humidity from either side
/// (`Climate::humidity_level`); over water the hotter the more goes
/// up, over land the hotter the more is burnt off; under low
/// pressure the air rises and cloud forms, under high it sinks and
/// clears; and a cloud raining in this climate
/// (`Climate::rains_above`) sheds.
pub const fn deepens_and_thins(climate: Climate, cloud: u32, over_water: bool) -> (u32, u32) {
    let humidity_cloud_level = climate.humidity_level();
    let deepen_parts = humidity_cloud_level.saturating_sub(cloud) * PARTS_A_LEVEL_FROM_HUMIDITY + PRESSURE_AT_REST.saturating_sub(climate.pressure) / PRESSURE_DIVIDES_ITS_GAP;
    let thin_parts = cloud.saturating_sub(humidity_cloud_level) * PARTS_A_LEVEL_FROM_HUMIDITY + climate.pressure.saturating_sub(PRESSURE_AT_REST) / PRESSURE_DIVIDES_ITS_GAP + if climate.rains(cloud) { PARTS_RAIN_SHEDS } else { 0 };
    if over_water { (deepen_parts + climate.temperature / EVAPORATION_DIVIDES_HEAT, thin_parts) } else { (deepen_parts, thin_parts + climate.temperature / BURNING_OFF_DIVIDES_HEAT) }
}

/// The cloud forms: that at level `cloud` over the tile at `sampled_tile`,
/// of climate `climate_here`, deepens or thins a level, or stays
/// ([`deepens_and_thins`]).
fn cloud_forms(tick_state: &mut SimulationTickState, sampled_tile: CellIndex, climate_here: Climate, cloud: u32) {
    // A tile is water's if most of its cells are under it.
    let over_water = cell_reads_and_writes::held_in_tile(tick_state, WET, sampled_tile) * 2 > (TILE_SIDE * TILE_SIDE) as u32;
    let (deepen_parts, thin_parts) = deepens_and_thins(climate_here, cloud, over_water);
    let drawn_part = tick_and_random_stream::random_stream(tick_state).draw_below(u64::from(WHOLE_PARTS)) as u32;
    if drawn_part < deepen_parts {
        if cloud < CLOUD.most_value() {
            cell_reads_and_writes::set_value_counted(tick_state, CLOUD, sampled_tile, cloud, cloud + 1, DEEPENED_COUNT);
        }
    } else if drawn_part < deepen_parts + thin_parts && cloud > 0 {
        cell_reads_and_writes::set_value_counted(tick_state, CLOUD, sampled_tile, cloud, cloud - 1, THINNED_COUNT);
    }
}

/// The rule, on one tile of the climate sampled -- each with
/// [`SAMPLE_CHANCE`], by whoever ticks the rule -- named by its
/// first cell, `sampled_tile`. The air moves, one way or the other:
/// pressure too far over a tile about it falls there
/// ([`pressure_falls`]), else the wind blows ([`wind_blows`]). Then
/// its cloud forms or clears ([`cloud_forms`]), unless a level of it
/// has just been blown off.
#[inline]
pub fn rule(tick_state: &mut SimulationTickState, sampled_tile: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED_COUNT] += 1;
    let (Some(climate), Some(cloud)) = (cell_reads_and_writes::value(tick_state, CLIMATE, sampled_tile), cell_reads_and_writes::value(tick_state, CLOUD, sampled_tile)) else {
        return;
    };
    let climate_here = Climate::of_number(climate);
    counts[RAINING_COUNT] += u64::from(climate_here.rains(cloud));
    if !pressure_falls(tick_state, sampled_tile, climate_here) && wind_blows(tick_state, sampled_tile, climate_here, cloud) {
        return;
    }
    cloud_forms(tick_state, sampled_tile, climate_here, cloud);
}
