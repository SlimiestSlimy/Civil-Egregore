//! Grass over dirt: each tick a cell of grass may spread onto a dirt
//! neighbour, or decay back to dirt the more grass is around it -- one
//! sampling pass for both (`docs/mc_rules.md`, "Grass").

use instructions::layers::{GRASS, WET};
use instructions::{cells, place_counted, CellIndex, Chance, RuleCounts, Turn, NEIGHBOURS};

/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: Chance = Chance::one_in(100_000);
/// The chance, each tick, that a cell of grass with grass all round
/// turns back to dirt.
pub const DECAY_CHANCE: Chance = Chance::one_in(200_000);
/// The chance, each tick, that a cell of grass is sampled: to spread or
/// to decay.
pub const SAMPLE_CHANCE: Chance = SPREAD_CHANCE.plus(DECAY_CHANCE);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 3] = ["sampled", "spreads", "decays"];
/// Cells of grass sampled.
pub const SAMPLED: usize = place_counted(&COUNTED, "sampled");
/// Spreads queued: two samples may spread onto one cell, which then
/// changes once.
pub const SPREADS: usize = place_counted(&COUNTED, "spreads");
/// Cells of grass turned back to dirt.
pub const DECAYS: usize = place_counted(&COUNTED, "decays");

/// The rule, on one superchunk's turn: every cell of grass chosen with
/// the chances of spreading and of decay together, in Morton order,
/// each seen to by [`cell`].
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> RuleCounts {
    let (sampled, mut counts) = cells::each_sampled(turn, GRASS, SAMPLE_CHANCE, samples, cell);
    counts[SAMPLED] = sampled as u64;
    counts
}

/// The rule, on one cell of grass chosen: it draws a neighbour, and
/// whether it tries to spread or to decay, and queues the write if the
/// neighbour lets it.
#[inline]
fn cell(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    let (dx, dy) = NEIGHBOURS[turn.random().below(NEIGHBOURS.len() as u64) as usize];
    let spreading = turn.random().chance_among(SPREAD_CHANCE, SAMPLE_CHANCE);
    // Stepped on the Morton index itself: no cartesian coordinates.
    let Some(neighbour) = cell.offset(dx, dy) else {
        return;
    };
    if spreading {
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if cells::lacks(turn, GRASS, neighbour) && !cells::holds(turn, WET, neighbour) {
            cells::set(turn, GRASS, neighbour);
            counts[SPREADS] += 1;
        }
    } else if cells::holds(turn, GRASS, neighbour) {
        cells::clear(turn, GRASS, cell);
        counts[DECAYS] += 1;
    }
}
