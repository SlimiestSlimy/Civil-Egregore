//! Grass over dirt: each tick a cell of grass may spread onto a dirt
//! neighbour, or -- far more rarely -- decay back to dirt by its
//! tile's climate, the drier and the hotter the likelier: one sampling
//! pass for both (`docs/sca_rules.md`, "Grass").

use crate::growing::{humid_enough_to_spread, too_harsh_to_live};
use actions::types::{GRASS, WET};
use actions::simulation_time::SimulationTime;
use actions::{cells, place_counted, this_tick, CellIndex, Chance, RuleCounts, Turn, NEIGHBOUR_OFFSETS};

/// The simulation time a cell of grass takes, on average, to try to
/// spread: what its chance each tick is worked out from, so the grass
/// grows as fast at any tick rate.
pub const MEAN_TIME_TO_SPREAD: SimulationTime = SimulationTime::from_seconds(2_600);
/// The simulation time a cell of grass in the harshest climate --
/// the driest, the hottest -- takes, on average, to turn back to dirt:
/// how rarely a cell of grass is held against its climate. In any
/// milder one it takes as much longer as the climate is milder.
pub const MEAN_TIME_TO_DECAY: SimulationTime = SimulationTime::from_seconds(20_800);
/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: Chance = MEAN_TIME_TO_SPREAD.chance_a_tick();
/// The chance, each tick, that a cell of grass is held against its
/// climate: that it turns back to dirt, in the harshest.
pub const DECAY_CHANCE: Chance = MEAN_TIME_TO_DECAY.chance_a_tick();
/// The chance, each tick, that a cell of grass is sampled: to spread or
/// to be held against its climate.
pub const SAMPLE_CHANCE: Chance = SPREAD_CHANCE.plus_disjoint(DECAY_CHANCE);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNT_NAMES: [&str; 3] = ["sampled", "spreads", "decays"];
/// Cells of grass sampled.
pub const SAMPLED_COUNT: usize = place_counted(&COUNT_NAMES, "sampled");
/// Cells grass spread onto: counted as each is applied, so two
/// samples spreading onto one cell are one spread.
pub const SPREADS_COUNT: usize = place_counted(&COUNT_NAMES, "spreads");
/// Cells of grass turned back to dirt by their climate: counted as
/// each is applied.
pub const DECAYS_COUNT: usize = place_counted(&COUNT_NAMES, "decays");

/// The rule, on one cell of grass sampled -- each with
/// [`SAMPLE_CHANCE`], the chances of spreading and of decay
/// together, by whoever ticks the rule: it draws whether it tries
/// to spread or is held against its climate. Spreading, it draws a
/// neighbour and queues the grass there if the neighbour lets it and
/// the air over it is humid enough this time
/// ([`humid_enough_to_spread`]);
/// held against its climate, it decays with a chance of the
/// harshness of its tile's ([`too_harsh_to_live`]).
#[inline]
pub fn rule(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED_COUNT] += 1;
    if this_tick::random_stream(turn).chance_among(SPREAD_CHANCE, SAMPLE_CHANCE) {
        let (dx, dy) = NEIGHBOUR_OFFSETS[this_tick::random_stream(turn).draw_below(NEIGHBOUR_OFFSETS.len() as u64) as usize];
        // Stepped on the Morton index itself: no cartesian coordinates.
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if let Some(neighbour_cell) = cell.at_cartesian_offset(dx, dy).filter(|&candidate_cell| cells::layer_lacks(turn, GRASS, candidate_cell) && !cells::layer_holds(turn, WET, candidate_cell))
            && humid_enough_to_spread(turn, neighbour_cell)
        {
            cells::set_counted(turn, GRASS, neighbour_cell, SPREADS_COUNT);
        }
    } else if too_harsh_to_live(turn, cell) {
        cells::clear_counted(turn, GRASS, cell, DECAYS_COUNT);
    }
}
