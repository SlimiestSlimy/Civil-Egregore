//! The weather: clouds over the climate, a tile 8 by 8 cells at a
//! time. A tile sampled has its cloud blown a step by its wind, or
//! deepened or thinned by its humidity, its heat and what is under it
//! (`docs/sca_rules.md`, "Weather").

use actions::simulation_time::SimulationTime;
use actions::types::{Climate, CLIMATE, CLOUD, MOST_HUMID, STRONGEST_WIND, WEATHER_TILE_SCALE, WET};
use actions::{cells, place_counted, this_tick, CellIndex, Chance, RuleCounts, Turn};

/// The simulation time from one sample of a tile to the next, on
/// average: what its chance each tick is worked out from, so the
/// weather goes as fast at any tick rate.
pub const MEAN_TIME_TO_SAMPLE: SimulationTime = SimulationTime::seconds(2);
/// The chance, each tick, that a tile is sampled.
pub const SAMPLE_CHANCE: Chance = MEAN_TIME_TO_SAMPLE.chance_a_tick();
/// What the chances of a sample are of: a cloud deepens or thins with
/// so many parts of this.
pub const WHOLE: u32 = 256;
/// The parts of [`WHOLE`] a cloud deepens with for each level it is
/// under its tile's humidity, and thins with for each level over it:
/// the more humid, the likelier a level more.
pub const PARTS_A_LEVEL_FROM_HUMIDITY: u32 = WHOLE / (MOST_HUMID + 1);
/// The temperature over this is the parts of [`WHOLE`] the cloud over
/// water deepens with: the hotter, the more of the water goes up.
pub const EVAPORATION_DIVIDES_HEAT: u32 = 16;
/// The temperature over this is the parts of [`WHOLE`] the cloud over
/// land thins with: the hotter, the more of it is burnt off.
pub const BURNING_OFF_DIVIDES_HEAT: u32 = 4;
/// The parts of [`WHOLE`] a raining cloud thins with: its water falls.
pub const PARTS_RAIN_SHEDS: u32 = WHOLE / 4;
/// Cells along the side of a tile.
const TILE_SIDE: i32 = 1 << WEATHER_TILE_SCALE;

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 5] = ["sampled", "deepened", "thinned", "blown", "raining"];
/// Tiles sampled.
pub const SAMPLED: usize = place_counted(&COUNTED, "sampled");
/// Clouds a level deeper: counted as each is applied.
pub const DEEPENED: usize = place_counted(&COUNTED, "deepened");
/// Clouds a level thinner: counted as each is applied.
pub const THINNED: usize = place_counted(&COUNTED, "thinned");
/// Levels of cloud blown to the next tile: counted as each is applied.
pub const BLOWN: usize = place_counted(&COUNTED, "blown");
/// Tiles sampled that were raining.
pub const RAINING: usize = place_counted(&COUNTED, "raining");

/// The parts of [`WHOLE`] with which the cloud at level `cloud` over
/// a tile of `climate` deepens a level, and with which it thins one:
/// towards the tile's humidity from either side; over water the
/// hotter the more goes up, over land the hotter the more is burnt
/// off; and a cloud raining in this climate
/// ([`Climate::rains_above`], of its humidity and its temperature)
/// sheds.
pub const fn deepens_and_thins(climate: Climate, cloud: u32, over_water: bool) -> (u32, u32) {
    let deepens = climate.humidity.saturating_sub(cloud) * PARTS_A_LEVEL_FROM_HUMIDITY;
    let thins = cloud.saturating_sub(climate.humidity) * PARTS_A_LEVEL_FROM_HUMIDITY + if climate.rains(cloud) { PARTS_RAIN_SHEDS } else { 0 };
    if over_water { (deepens + climate.temperature / EVAPORATION_DIVIDES_HEAT, thins) } else { (deepens, thins + climate.temperature / BURNING_OFF_DIVIDES_HEAT) }
}

/// The rule, on one tile of the climate sampled -- each with
/// [`SAMPLE_CHANCE`], by whoever ticks the rule -- named by its
/// first cell, `sampled`: its wind
/// may blow a level of its cloud to the tile it blows towards --
/// the stronger the likelier, and only where that tile has room --
/// or else the cloud deepens or thins a level
/// ([`deepens_and_thins`]).
#[inline]
pub fn rule(turn: &mut Turn, sampled: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED] += 1;
    let (Some(climate), Some(cloud)) = (cells::value(turn, CLIMATE, sampled), cells::value(turn, CLOUD, sampled)) else {
        return;
    };
    let climate = Climate::of_number(climate);
    counts[RAINING] += u64::from(climate.rains(cloud));
    if let Some((across, down)) = climate.wind.towards().filter(|_| cloud > 0)
        && this_tick::random(turn).below(u64::from(STRONGEST_WIND + 1)) < u64::from(climate.wind.strength())
        && let Some(towards) = sampled.offset(across * TILE_SIDE, down * TILE_SIDE)
        && let Some(there) = cells::value(turn, CLOUD, towards).filter(|&there| there < CLOUD.most())
    {
        cells::move_one(turn, CLOUD, sampled, cloud, towards, there, BLOWN);
        return;
    }
    // A tile is water's if most of its cells are under it.
    let over_water = cells::held_in_tile(turn, WET, sampled) * 2 > (TILE_SIDE * TILE_SIDE) as u32;
    let (deepens, thins) = deepens_and_thins(climate, cloud, over_water);
    let drawn = this_tick::random(turn).below(u64::from(WHOLE)) as u32;
    if drawn < deepens {
        if cloud < CLOUD.most() {
            cells::set_value_counted(turn, CLOUD, sampled, cloud, cloud + 1, DEEPENED);
        }
    } else if drawn < deepens + thins && cloud > 0 {
        cells::set_value_counted(turn, CLOUD, sampled, cloud, cloud - 1, THINNED);
    }
}
