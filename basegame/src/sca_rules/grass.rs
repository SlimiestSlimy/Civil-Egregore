//! Grass over dirt, by itself: each tick a cell of grass may spread onto
//! a dirt neighbour, or -- half as often -- turn back to dirt where its
//! neighbours crowd it: one sampling pass for both
//! (`docs/sca_rules.md`, "Grass").

use crate::base_types::BaseTypes;
use protocol::server_types::simulation_time::SimulationTime;
use server::simulation::simulation_api::{on_each_sampled, RuleCode, cell_reads_and_writes, tick_and_random_stream, CellIndex, Chance, SimulationTickState, NEIGHBOUR_OFFSETS};
use server::simulation::simulation_api::rule_counts::{place_counted, RuleCounts};

/// The simulation time a cell of grass takes, on average, to try to
/// spread: what its chance each tick is worked out from, so the grass
/// grows as fast at any tick rate.
pub const MEAN_TIME_TO_SPREAD: SimulationTime = SimulationTime::from_milliseconds(390_625);
/// The simulation time a cell of grass with grass all round takes, on
/// average, to turn back to dirt.
pub const MEAN_TIME_TO_DECAY: SimulationTime = SimulationTime::from_milliseconds(781_250);
/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: Chance = MEAN_TIME_TO_SPREAD.chance_a_tick();
/// The chance, each tick, that a cell of grass with grass all round
/// turns back to dirt.
pub const DECAY_CHANCE: Chance = MEAN_TIME_TO_DECAY.chance_a_tick();
/// The chance, each tick, that a cell of grass is sampled: to spread or
/// to decay.
pub const SAMPLE_CHANCE: Chance = SPREAD_CHANCE.plus_disjoint(DECAY_CHANCE);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNT_NAMES: [&str; 3] = ["sampled", "spreads", "decays"];
/// What runs the grass's rule, as it is registered with `base_types`:
/// [`rule`] on each sample of its master, chosen with [`SAMPLE_CHANCE`].
pub fn code(base_types: BaseTypes) -> RuleCode {
    RuleCode::new(&COUNT_NAMES, move |tick_state, context| on_each_sampled(tick_state, context.master, SAMPLE_CHANCE, |tick_state, cell, counts| rule(&base_types, tick_state, cell, counts)))
}
/// Cells of grass sampled.
pub const SAMPLED_COUNT: usize = place_counted(&COUNT_NAMES, "sampled");
/// Cells grass spread onto: counted as each is applied, so two
/// samples spreading onto one cell are one spread.
pub const SPREADS_COUNT: usize = place_counted(&COUNT_NAMES, "spreads");
/// Cells of grass turned back to dirt: counted as each is applied.
pub const DECAYS_COUNT: usize = place_counted(&COUNT_NAMES, "decays");

/// The rule, on one cell of grass sampled -- each with
/// [`SAMPLE_CHANCE`], the chances of spreading and of decay together,
/// by whoever ticks the rule: it draws a neighbour, and whether it
/// tries to spread or to decay. Spreading, it queues the grass on the
/// neighbour if the neighbour is dry dirt; decaying, it turns back to
/// dirt if the neighbour is grass -- crowded out.
#[inline]
pub fn rule(base_types: &BaseTypes, tick_state: &mut SimulationTickState, cell: CellIndex, counts: &mut RuleCounts) {
    counts[SAMPLED_COUNT] += 1;
    let (dx, dy) = NEIGHBOUR_OFFSETS[tick_and_random_stream::random_stream(tick_state).draw_below(NEIGHBOUR_OFFSETS.len() as u64) as usize];
    let spreading = tick_and_random_stream::random_stream(tick_state).chance_among(SPREAD_CHANCE, SAMPLE_CHANCE);
    // Stepped on the Morton index itself: no cartesian coordinates.
    let Some(neighbour_cell) = cell.at_cartesian_offset(dx, dy) else {
        return;
    };
    if spreading {
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if cell_reads_and_writes::layer_lacks(tick_state, base_types.grass, neighbour_cell) && !cell_reads_and_writes::layer_holds(tick_state, base_types.wet, neighbour_cell) {
            cell_reads_and_writes::set_counted(tick_state, base_types.grass, neighbour_cell, SPREADS_COUNT);
        }
    } else if cell_reads_and_writes::layer_holds(tick_state, base_types.grass, neighbour_cell) {
        cell_reads_and_writes::clear_counted(tick_state, base_types.grass, cell, DECAYS_COUNT);
    }
}
