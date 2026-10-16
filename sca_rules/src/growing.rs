//! What grass and trees share: how the climate lets what grows
//! spread, and how it kills it.

use actions::types::{Climate, CLIMATE, HOTTEST_TEMPERATURE, MOST_HUMID};
use actions::{cells, this_tick, CellIndex, Turn};

/// How harsh `climate` is to what grows, of [`HARSHEST_CLIMATE`]: how dry it
/// is times how hot -- none where it is the most humid, or the
/// coldest.
pub const fn climate_harshness(climate: Climate) -> u32 {
    (MOST_HUMID - climate.humidity) * climate.temperature
}

/// The harshest a climate is to what grows: the driest, the hottest.
pub const HARSHEST_CLIMATE: u32 = MOST_HUMID * HOTTEST_TEMPERATURE;

/// Whether the air over `onto_cell` is humid enough, this time, for what
/// grows to spread there: drawn, as likely as its tile is humid --
/// always in the most humid, never in the driest. Where no climate is
/// hot there is none to hold it back.
pub fn humid_enough_to_spread(turn: &mut Turn, onto_cell: CellIndex) -> bool {
    match cells::value(turn, CLIMATE, onto_cell) {
        Some(climate) => this_tick::random_stream(turn).draw_below(u64::from(MOST_HUMID)) < u64::from(Climate::of_number(climate).humidity),
        None => true,
    }
}

/// Whether the climate over `cell` is too harsh, this time, for what
/// grows there to live: drawn, as likely as it is harsh
/// ([`climate_harshness`]) -- always in the driest and hottest, never in the
/// most humid or the coldest, nor where no climate is hot.
pub fn too_harsh_to_live(turn: &mut Turn, cell: CellIndex) -> bool {
    match cells::value(turn, CLIMATE, cell) {
        Some(climate) => this_tick::random_stream(turn).draw_below(u64::from(HARSHEST_CLIMATE)) < u64::from(climate_harshness(Climate::of_number(climate))),
        None => false,
    }
}
