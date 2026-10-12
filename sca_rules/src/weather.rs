//! The weather: clouds over the climate, a tile 8 by 8 cells at a
//! time. A tile sampled has its cloud blown a step by its wind, or
//! deepened or thinned by its humidity, its heat and what is under it
//! (`docs/sca_rules.md`, "Weather").

use actions::simulation_time::SimulationTime;
use actions::types::{passes_a_rise, pressure_falls_by, Climate, CLIMATE, CLOUD, ELEVATION, HIGHEST_PRESSURE, HUMIDITY_A_LEVEL_OF_CLOUD, PRESSURE_AT_REST, STRONGEST_WIND, TILES_ABOUT, WEATHER_TILE_SCALE, WET};
use actions::{cells, place_counted, this_tick, CellIndex, Chance, RuleCounts, Turn};

/// The simulation time from one sample of a tile to the next, on
/// average: what its chance each tick is worked out from, so the
/// weather goes as fast at any tick rate.
pub const MEAN_TIME_TO_SAMPLE: SimulationTime = SimulationTime::seconds(2);
/// The chance, each tick, that a tile is sampled.
pub const SAMPLE_CHANCE: Chance = MEAN_TIME_TO_SAMPLE.chance_a_tick();
/// What a sample's chances are parts of: one number drawn under it
/// says what the tile does.
pub const WHOLE: u32 = 256;
/// The parts with which a cloud deepens for each level it is under
/// its tile's humidity, and thins for each level over it.
pub const PARTS_A_LEVEL_FROM_HUMIDITY: u32 = WHOLE / HUMIDITY_A_LEVEL_OF_CLOUD;
/// Over water the temperature, divided by this, is parts with which
/// the cloud deepens: what the heat takes up of the water.
pub const EVAPORATION_DIVIDES_HEAT: u32 = 16;
/// Over land the temperature, divided by this, is parts with which
/// the cloud thins: what the heat burns off.
pub const BURNING_OFF_DIVIDES_HEAT: u32 = 4;
/// The parts with which a raining cloud thins besides.
pub const PARTS_RAIN_SHEDS: u32 = WHOLE / 4;
/// How far a tile's pressure is from rest, divided by this, is parts
/// with which its cloud deepens -- under rest, air rising -- or
/// thins -- over it, air sinking.
pub const PRESSURE_DIVIDES_ITS_GAP: u32 = 2;
/// Cells along a side of a tile of the weather.
const TILE_SIDE: i32 = 1 << WEATHER_TILE_SCALE;

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 8] = ["sampled", "deepened", "thinned", "blown", "raining", "carried", "pushed", "evened"];
/// Tiles sampled.
pub const SAMPLED: usize = place_counted(&COUNTED, "sampled");
/// Levels of cloud added: counted as each is applied.
pub const DEEPENED: usize = place_counted(&COUNTED, "deepened");
/// Levels of cloud taken.
pub const THINNED: usize = place_counted(&COUNTED, "thinned");
/// Levels of cloud a wind moved to the tile it blew to.
pub const BLOWN: usize = place_counted(&COUNTED, "blown");
/// Tiles sampled that were raining.
pub const RAINING: usize = place_counted(&COUNTED, "raining");
/// Tiles a wind carried humidity, heat or itself onto.
pub const CARRIED: usize = place_counted(&COUNTED, "carried");
/// Units of pressure a wind pushed onto the tile it blew to.
pub const PUSHED: usize = place_counted(&COUNTED, "pushed");
/// Units of pressure that fell from a tile to a lower one beside it.
pub const EVENED: usize = place_counted(&COUNTED, "evened");

/// The simulation time a wind blowing takes to carry something of its
/// tile's climate to the next, on average, were it to reach it every
/// sample: slowly, beside the air and the clouds it moves.
pub const MEAN_TIME_TO_CARRY_CLIMATE: SimulationTime = SimulationTime::seconds(60);
/// The chance, each sample its wind reaches a tile in, that it
/// carries climate and not air.
pub const CARRY_CHANCE: Chance = MEAN_TIME_TO_CARRY_CLIMATE.chance_each(MEAN_TIME_TO_SAMPLE);
/// What a wind carries of a climate, one of them drawn each time:
/// its humidity, its temperature, itself.
const CARRIED_OF_A_CLIMATE: u64 = 3;

/// The tile a step of `(across, down)` tiles from `tile`, and its
/// climate: none past the world's edge or where it is not loaded.
fn beside(turn: &Turn, tile: CellIndex, (across, down): (i32, i32)) -> Option<(CellIndex, Climate)> {
    let beside = tile.offset(across * TILE_SIDE, down * TILE_SIDE)?;
    cells::value(turn, CLIMATE, beside).map(|climate| (beside, Climate::of_number(climate)))
}

/// Whether the tile at `tile` has a tile about it that is not
/// loaded, or is past the world's edge: its wind is what comes in
/// from there, and nothing inside turns it.
fn at_the_border_of_what_is_loaded(turn: &Turn, tile: CellIndex) -> bool {
    TILES_ABOUT.iter().any(|&step| beside(turn, tile, step).is_none())
}

/// The parts of `whole` with which air goes from the tile at `from`
/// to the one at `to`: all of it on the level or downhill, the less
/// the steeper the rise (`passes_a_rise`) -- the one resistance of
/// the land, to pressure and to wind alike.
fn passes(turn: &Turn, from: CellIndex, to: CellIndex, whole: u32) -> u32 {
    let high = |tile| cells::value(turn, ELEVATION, tile).unwrap_or(0);
    passes_a_rise(high(to).saturating_sub(high(from)), whole)
}

/// A unit of pressure goes from the tile at `from`, whose climate
/// `here` becomes `left` but for that unit, to the one at `to`, of
/// climate `there`: both climates under both compares, so no air is
/// made or lost wherever a border falls between them.
fn give_pressure(turn: &mut Turn, (from, here, left): (CellIndex, Climate, Climate), (to, there): (CellIndex, Climate), counted: usize) {
    let (left, got) = (Climate { pressure: here.pressure - 1, ..left }, Climate { pressure: there.pressure + 1, ..there });
    cells::change_both(turn, CLIMATE, (from, here.number(), left.number()), (to, there.number(), got.number()), counted);
}

/// Pressure falls: the tile at `sampled`, of climate `here`, gives a
/// unit of its pressure to one of the tiles about it far enough
/// under it (`pressure_falls_by`), drawn among them by how far under
/// each is and how little the land rises to it ([`passes`]) -- the
/// lowest the likeliest, none higher ever, and up a slope less
/// often. The air leaving turns the tile's own wind that way
/// (`Wind::pushed`), unless it is at the border of what is loaded.
/// Whether a unit went.
fn pressure_falls(turn: &mut Turn, sampled: CellIndex, here: Climate) -> bool {
    let about = TILES_ABOUT.map(|step| beside(turn, sampled, step));
    let falls = about.map(|beside| beside.map_or(0, |(_, there)| pressure_falls_by(here.pressure, there.pressure)));
    let whole: u32 = falls.iter().sum();
    if whole == 0 || here.pressure == 0 {
        return false;
    }
    // One number drawn over all that would fall on level land: what the land holds back of each is where nothing goes.
    let mut drawn = this_tick::random(turn).below(u64::from(whole * WHOLE)) as u32;
    for (way, &falls) in falls.iter().enumerate().filter(|(_, falls)| **falls > 0) {
        let Some((towards, there)) = about[way] else {
            continue;
        };
        if drawn < falls * passes(turn, sampled, towards, WHOLE) {
            let wind = if about.iter().all(Option::is_some) { here.wind.pushed(TILES_ABOUT[way].0, TILES_ABOUT[way].1) } else { here.wind };
            give_pressure(turn, (sampled, here, Climate { wind, ..here }), (towards, there), EVENED);
            return true;
        }
        drawn = drawn.saturating_sub(falls * WHOLE);
    }
    false
}

/// The wind of the tile of climate `here` carries something of it
/// onto the tile at `towards`, of climate `there`: its humidity, its
/// temperature or its own vector, one drawn, each a step nearer the
/// tile it comes from (`Climate::with_humidity_carried_from` and the
/// two beside it). A tile at the border of what is loaded keeps its
/// wind. Nothing is queued where the two are alike already.
fn carry(turn: &mut Turn, here: Climate, towards: CellIndex, there: Climate) {
    let carried = match this_tick::random(turn).below(CARRIED_OF_A_CLIMATE) {
        0 => there.with_humidity_carried_from(here),
        1 => there.with_temperature_carried_from(here),
        _ if at_the_border_of_what_is_loaded(turn, towards) => there,
        _ => there.with_wind_carried_from(here),
    };
    if carried != there {
        cells::set_value_counted(turn, CLIMATE, towards, there.number(), carried.number(), CARRIED);
    }
}

/// The wind blows: that of the tile at `sampled`, of climate `here`
/// under `cloud`, reaches one of the eight tiles about it -- drawn by
/// its vector, the stronger the likelier (`Wind::step_drawn`) -- if
/// the land lets it ([`passes`]). There it carries climate, now and
/// then ([`carry`]), or else pushes a unit of its tile's pressure
/// onto that tile -- so winds that meet on a tile raise it -- and
/// with it a level of its cloud, if it has one and there is room.
/// Whether a level of cloud went.
fn wind_blows(turn: &mut Turn, sampled: CellIndex, here: Climate, cloud: u32) -> bool {
    let strongest = STRONGEST_WIND as u64 + 1;
    let (drawn_across, drawn_down) = (this_tick::random(turn).below(strongest) as u32, this_tick::random(turn).below(strongest) as u32);
    let Some((towards, there)) = here.wind.step_drawn(drawn_across, drawn_down).and_then(|step| beside(turn, sampled, step)) else {
        return false;
    };
    if this_tick::random(turn).below(u64::from(WHOLE)) as u32 >= passes(turn, sampled, towards, WHOLE) {
        return false;
    }
    if this_tick::random(turn).chance(CARRY_CHANCE) {
        carry(turn, here, towards, there);
        return false;
    }
    if here.pressure > 0 && there.pressure < HIGHEST_PRESSURE {
        give_pressure(turn, (sampled, here, here), (towards, there), PUSHED);
    }
    if cloud > 0
        && let Some(over_there) = cells::value(turn, CLOUD, towards).filter(|&over_there| over_there < CLOUD.most())
    {
        cells::move_one(turn, CLOUD, sampled, cloud, towards, over_there, BLOWN);
        return true;
    }
    false
}

/// The parts of [`WHOLE`] with which the cloud at level `cloud` over
/// a tile of `climate` deepens a level, and with which it thins one:
/// towards the tile's humidity from either side
/// (`Climate::humidity_level`); over water the hotter the more goes
/// up, over land the hotter the more is burnt off; under low
/// pressure the air rises and cloud forms, under high it sinks and
/// clears; and a cloud raining in this climate
/// (`Climate::rains_above`) sheds.
pub const fn deepens_and_thins(climate: Climate, cloud: u32, over_water: bool) -> (u32, u32) {
    let level = climate.humidity_level();
    let deepens = level.saturating_sub(cloud) * PARTS_A_LEVEL_FROM_HUMIDITY + PRESSURE_AT_REST.saturating_sub(climate.pressure) / PRESSURE_DIVIDES_ITS_GAP;
    let thins = cloud.saturating_sub(level) * PARTS_A_LEVEL_FROM_HUMIDITY + climate.pressure.saturating_sub(PRESSURE_AT_REST) / PRESSURE_DIVIDES_ITS_GAP + if climate.rains(cloud) { PARTS_RAIN_SHEDS } else { 0 };
    if over_water { (deepens + climate.temperature / EVAPORATION_DIVIDES_HEAT, thins) } else { (deepens, thins + climate.temperature / BURNING_OFF_DIVIDES_HEAT) }
}

/// The cloud forms: that at level `cloud` over the tile at `sampled`,
/// of climate `here`, deepens or thins a level, or stays
/// ([`deepens_and_thins`]).
fn cloud_forms(turn: &mut Turn, sampled: CellIndex, here: Climate, cloud: u32) {
    // A tile is water's if most of its cells are under it.
    let over_water = cells::held_in_tile(turn, WET, sampled) * 2 > (TILE_SIDE * TILE_SIDE) as u32;
    let (deepens, thins) = deepens_and_thins(here, cloud, over_water);
    let drawn = this_tick::random(turn).below(u64::from(WHOLE)) as u32;
    if drawn < deepens {
        if cloud < CLOUD.most() {
            cells::set_value_counted(turn, CLOUD, sampled, cloud, cloud + 1, DEEPENED);
        }
    } else if drawn < deepens + thins && cloud > 0 {
        cells::set_value_counted(turn, CLOUD, sampled, cloud, cloud - 1, THINNED);
    }
}

/// The rule, on one tile of the climate sampled -- each with
/// [`SAMPLE_CHANCE`], by whoever ticks the rule -- named by its
/// first cell, `sampled`. The air moves, one way or the other:
/// pressure too far over a tile about it falls there
/// ([`pressure_falls`]), else the wind blows ([`wind_blows`]). Then
/// its cloud forms or clears ([`cloud_forms`]), unless a level of it
/// has just been blown off.
#[inline]
pub fn rule(turn: &mut Turn, sampled: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED] += 1;
    let (Some(climate), Some(cloud)) = (cells::value(turn, CLIMATE, sampled), cells::value(turn, CLOUD, sampled)) else {
        return;
    };
    let here = Climate::of_number(climate);
    counts[RAINING] += u64::from(here.rains(cloud));
    if !pressure_falls(turn, sampled, here) && wind_blows(turn, sampled, here, cloud) {
        return;
    }
    cloud_forms(turn, sampled, here, cloud);
}
