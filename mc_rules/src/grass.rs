//! Grass over dirt: Civil Egregore's first rule. Each tick every cell of grass
//! may spread onto a dirt neighbour, or decay back to dirt the more grass
//! is around it:
//!
//! - **Spreading**: a cell of grass tries to spread with
//!   [`SPREAD_CHANCE`], onto one of its eight neighbours drawn at
//!   random, if that one is dirt -- a cell with no grass -- and not
//!   under water.
//! - **Decay**: a cell of grass with `k` of its eight neighbours grass
//!   turns back to dirt with `k / 8` of [`DECAY_CHANCE`]: none with no
//!   grass around, the whole chance with grass all round.
//!
//! One sampling pass serves both, and no sample is wasted: every cell of
//! grass is sampled with the two chances together, and each sample
//! draws one neighbour and which of the two it tries -- spreading, in
//! [`SPREAD_CHANCE`] of the sum, else decay. Decay so happens when the
//! neighbour drawn is grass: `k / 8` of the time, as asked, from one
//! neighbour read rather than eight.
//!
//! The rule runs on each superchunk in a tick's first phase
//! (`Simulation::tick`): its writes are queued as
//! the samples come, in Morton order -- grass spreading over a border
//! into the neighbour's queue -- and applied in the second phase, so
//! every sample reads the world as the tick found it. The two never
//! touch one cell in a tick: decay clears cells that were grass,
//! spreading fills cells that were dirt.

use instructions::layers::{GRASS, WET};
use instructions::{read, write, CellIndex, RuleCounts, Turn, NEIGHBOURS};

/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: f64 = 0.000_01;
/// The chance, each tick, that a cell of grass with grass all round
/// turns back to dirt.
pub const DECAY_CHANCE: f64 = 0.000_005;

/// What the rule counts, each named at its place in its [`RuleCounts`].
pub const COUNTED: [&str; 3] = ["sampled", "spreads", "decays"];
/// Cells of grass sampled.
pub const SAMPLED: usize = 0;
/// Spreads queued: two samples may spread onto one cell, which then
/// changes once.
pub const SPREADS: usize = 1;
/// Cells of grass turned back to dirt.
pub const DECAYS: usize = 2;

/// The rule, on one superchunk's turn: every cell of grass chosen with
/// the chances of spreading and of decay together, in Morton order,
/// each seen to by [`cell`].
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> RuleCounts {
    let (sampled, mut counts) = read::cells::each_sampled(turn, GRASS, SPREAD_CHANCE + DECAY_CHANCE, samples, cell);
    counts[SAMPLED] = sampled as u64;
    counts
}

/// The rule, on one cell of grass chosen: it draws a neighbour, and
/// whether it tries to spread or to decay, and queues the write if the
/// neighbour lets it.
#[inline]
fn cell(turn: &mut Turn, cell: CellIndex, counts: &mut RuleCounts) {
    let spread_share = SPREAD_CHANCE / (SPREAD_CHANCE + DECAY_CHANCE);
    let (dx, dy) = NEIGHBOURS[turn.random().below(NEIGHBOURS.len() as u64) as usize];
    let spreading = turn.random().unit() <= spread_share;
    // Stepped on the Morton index itself: no cartesian coordinates.
    let Some(neighbour) = cell.offset(dx, dy) else {
        return;
    };
    if spreading {
        // Dirt is a cell with no grass on it: no layer of its own.
        // And grass does not spread under water; a world with no water has none.
        if read::cells::lacks(turn, GRASS, neighbour) && !read::cells::holds(turn, WET, neighbour) {
            write::cells::set(turn, GRASS, neighbour);
            counts[SPREADS] += 1;
        }
    } else if read::cells::holds(turn, GRASS, neighbour) {
        write::cells::clear(turn, GRASS, cell);
        counts[DECAYS] += 1;
    }
}
