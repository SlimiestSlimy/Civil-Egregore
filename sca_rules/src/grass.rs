//! Grass over dirt: each tick a cell of grass may spread onto a dirt
//! neighbour, or -- far more rarely -- decay back to dirt by its
//! tile's climate, the drier and the hotter the likelier: one sampling
//! pass for both (`docs/sca_rules.md`, "Grass").

use actions::types::{Climate, CLIMATE, GRASS, HOTTEST, MOST_HUMID, WET};
use actions::simulation_time::SimulationTime;
use actions::{cells, place_counted, this_tick, CellIndex, Chance, RuleCounts, Turn, NEIGHBOURS};

/// The simulation time a cell of grass takes, on average, to try to
/// spread: what its chance each tick is worked out from, so the grass
/// grows as fast at any tick rate.
pub const MEAN_TIME_TO_SPREAD: SimulationTime = SimulationTime::milliseconds(390_625);
/// The simulation time a cell of grass in the harshest climate --
/// the driest, the hottest -- takes, on average, to turn back to dirt:
/// how rarely a cell of grass is held against its climate. In any
/// milder one it takes as much longer as the climate is milder.
pub const MEAN_TIME_TO_DECAY: SimulationTime = SimulationTime::milliseconds(3_125_000);
/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: Chance = MEAN_TIME_TO_SPREAD.chance_a_tick();
/// The chance, each tick, that a cell of grass is held against its
/// climate: that it turns back to dirt, in the harshest.
pub const DECAY_CHANCE: Chance = MEAN_TIME_TO_DECAY.chance_a_tick();
/// The chance, each tick, that a cell of grass is sampled: to spread or
/// to be held against its climate.
pub const SAMPLE_CHANCE: Chance = SPREAD_CHANCE.plus(DECAY_CHANCE);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 3] = ["sampled", "spreads", "decays"];
/// Cells of grass sampled.
pub const SAMPLED: usize = place_counted(&COUNTED, "sampled");
/// Cells grass spread onto: counted as each is applied, so two
/// samples spreading onto one cell are one spread.
pub const SPREADS: usize = place_counted(&COUNTED, "spreads");
/// Cells of grass turned back to dirt by their climate: counted as
/// each is applied.
pub const DECAYS: usize = place_counted(&COUNTED, "decays");

/// The rule, on one superchunk's turn: every cell of grass chosen with
/// the chances of spreading and of decay together, in Morton order,
/// each seen to by [`cell`].
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> RuleCounts {
    let (sampled, mut counts) = cells::each_sampled(turn, GRASS, SAMPLE_CHANCE, samples, cell);
    counts[SAMPLED] = sampled as u64;
    counts
}

/// How harsh `climate` is to grass, of [`HARSHEST`]: how dry it is
/// times how hot -- none where it is the most humid, or the coldest.
pub const fn harshness(climate: Climate) -> u32 {
    (MOST_HUMID - climate.humidity) * climate.temperature
}

/// The harshest a climate is to grass: the driest, the hottest.
pub const HARSHEST: u32 = MOST_HUMID * HOTTEST;

/// The rule, on one cell of grass chosen: it draws whether it tries
/// to spread or is held against its climate. Spreading, it draws a
/// neighbour and queues the grass there if the neighbour lets it;
/// held against its climate, it decays with a chance of the
/// [`harshness`] of its tile's.
#[inline]
fn cell(turn: &mut Turn, cell: CellIndex, _counts: &mut RuleCounts) {
    if this_tick::random(turn).chance_among(SPREAD_CHANCE, SAMPLE_CHANCE) {
        let (dx, dy) = NEIGHBOURS[this_tick::random(turn).below(NEIGHBOURS.len() as u64) as usize];
        // Stepped on the Morton index itself: no cartesian coordinates.
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if let Some(neighbour) = cell.offset(dx, dy).filter(|&neighbour| cells::lacks(turn, GRASS, neighbour) && !cells::holds(turn, WET, neighbour)) {
            cells::set_counted(turn, GRASS, neighbour, SPREADS);
        }
    } else if let Some(climate) = cells::value(turn, CLIMATE, cell)
        && this_tick::random(turn).below(u64::from(HARSHEST)) < u64::from(harshness(Climate::of_number(climate)))
    {
        cells::clear_counted(turn, GRASS, cell, DECAYS);
    }
}
