//! Grass over dirt: each tick a cell of grass may spread onto a dirt
//! neighbour, or -- far more rarely -- decay back to dirt by its
//! tile's climate, the drier and the hotter the likelier: one sampling
//! pass for both (`docs/sca_rules.md`, "Grass").

use crate::growing::{humid_enough_to_spread, too_harsh_to_live};
use base_types::{GRASS, WET};
use simulation::simulation_api::simulation_time::SimulationTime;
use simulation::simulation_api::{on_each_sampled, RuleCode, cell_reads_and_writes, place_counted, tick_and_random_stream, CellIndex, Chance, RuleCounts, SimulationTickState, NEIGHBOUR_OFFSETS};

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
/// What runs the grass's rule, as it is registered: [`rule`] on each
/// sample of its master, chosen with [`SAMPLE_CHANCE`].
pub const CODE: RuleCode = RuleCode { count_names: &COUNT_NAMES, run: |tick_state, context| on_each_sampled(tick_state, context.master, SAMPLE_CHANCE, rule) };
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
pub fn rule(tick_state: &mut SimulationTickState, cell: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED_COUNT] += 1;
    if tick_and_random_stream::random_stream(tick_state).chance_among(SPREAD_CHANCE, SAMPLE_CHANCE) {
        let (dx, dy) = NEIGHBOUR_OFFSETS[tick_and_random_stream::random_stream(tick_state).draw_below(NEIGHBOUR_OFFSETS.len() as u64) as usize];
        // Stepped on the Morton index itself: no cartesian coordinates.
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if let Some(neighbour_cell) = cell.at_cartesian_offset(dx, dy).filter(|&candidate_cell| cell_reads_and_writes::layer_lacks(tick_state, GRASS, candidate_cell) && !cell_reads_and_writes::layer_holds(tick_state, WET, candidate_cell))
            && humid_enough_to_spread(tick_state, neighbour_cell)
        {
            cell_reads_and_writes::set_counted(tick_state, GRASS, neighbour_cell, SPREADS_COUNT);
        }
    } else if too_harsh_to_live(tick_state, cell) {
        cell_reads_and_writes::clear_counted(tick_state, GRASS, cell, DECAYS_COUNT);
    }
}
