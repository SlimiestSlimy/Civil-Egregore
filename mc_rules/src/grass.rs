//! Grass over dirt: TileSim's first rule. Each tick every cell of grass
//! may spread onto a dirt neighbour, or decay back to dirt the more grass
//! is around it:
//!
//! - **Spreading**: a cell of grass tries to spread with
//!   [`SPREAD_CHANCE`], onto one of its eight neighbours drawn at
//!   random, if that one is dirt.
//! - **Decay**: a cell of grass with `k` of its eight neighbours grass
//!   turns back to dirt with `k / 8` of [`DECAY_CHANCE`]: none with no
//!   grass around, the whole chance with grass all round.
//!
//! Grass grows only in a circle three superchunks across about the
//! middle of the world's origin superchunk ([`grows_at`]): a superchunk
//! the circle misses samples nothing, and a cell of grass outside it
//! never changes. Grass elsewhere lies as it was made.
//!
//! One sampling pass serves both, and no sample is wasted: every cell of
//! grass is sampled with the two chances together, and each sample
//! draws one neighbour and which of the two it tries -- spreading, in
//! [`SPREAD_CHANCE`] of the sum, else decay. Decay so happens when the
//! neighbour drawn is grass: `k / 8` of the time, as asked, from one
//! neighbour read rather than eight.
//!
//! The rule runs on each superchunk in a tick's first phase
//! ([`simulation::Simulation::tick`]): its writes are queued as
//! the samples come, in Morton order -- grass spreading over a border
//! into the neighbour's queue -- and applied in the second phase, so
//! every sample reads the world as the tick found it. The two never
//! touch one cell in a tick: decay clears cells that were grass,
//! spreading fills cells that were dirt.

use bitplane_manager::{BitmapArena, Write, WriteOp};
use simulation::entity_store::Entities;
use simulation::{Simulation, Turn, TickReport};
use chunk_storage::mock::{DIRT, GRASS};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOURS, SUPERCHUNK_SIDE_CELLS, WORLD_SIDE_SUPERCHUNKS};
use std::ops::AddAssign;

/// The chance, each tick, that a cell of grass tries to spread.
pub const SPREAD_CHANCE: f64 = 0.000_01;
/// The chance, each tick, that a cell of grass with grass all round
/// turns back to dirt.
pub const DECAY_CHANCE: f64 = 0.000_02;

/// The middle of the circle grass grows in: the middle cell of the
/// world's origin superchunk ([`WORLD_MIDDLE`]).
pub const GROWING_CENTRE: CellCartesian = {
    // WORLD_MIDDLE's coordinates, in superchunks, are both this.
    let middle = WORLD_SIDE_SUPERCHUNKS / 2 * SUPERCHUNK_SIDE_CELLS + SUPERCHUNK_SIDE_CELLS / 2;
    CellCartesian { x: middle, y: middle }
};
/// The radius of the circle grass grows in, in cells: three superchunks
/// across.
pub const GROWING_RADIUS: u32 = 3 * SUPERCHUNK_SIDE_CELLS / 2;

/// Whether grass grows at `cell`: no farther than [`GROWING_RADIUS`]
/// from [`GROWING_CENTRE`], centre to centre.
pub fn grows_at(cell: CellIndex) -> bool {
    let CellCartesian { x, y } = cell.cartesian();
    let (dx, dy) = (x.abs_diff(GROWING_CENTRE.x) as u64, y.abs_diff(GROWING_CENTRE.y) as u64);
    dx * dx + dy * dy <= GROWING_RADIUS as u64 * GROWING_RADIUS as u64
}

/// Whether grass grows anywhere in `superchunk`: the circle reaches its
/// nearest cell to [`GROWING_CENTRE`].
pub fn grows_in(superchunk: SuperchunkIndex) -> bool {
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let nearest = |centre: u32, first: u32| centre.clamp(first, first + (SUPERCHUNK_SIDE_CELLS - 1));
    grows_at(CellCartesian { x: nearest(GROWING_CENTRE.x, left), y: nearest(GROWING_CENTRE.y, top) }.into())
}

/// What the rule did in a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GrassCounts {
    /// Cells of grass sampled.
    pub sampled: usize,
    /// Spreads queued: two samples may spread onto one cell, which then
    /// changes once.
    pub spreads: usize,
    /// Cells of grass turned back to dirt.
    pub decays: usize,
}

impl AddAssign for GrassCounts {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.sampled += other.sampled;
        self.spreads += other.spreads;
        self.decays += other.decays;
    }
}

/// One tick of grass over every superchunk with a bitmap in use, on
/// `simulation`'s threads -- `seed`, the world's, seeding a superchunk's
/// random stream the first tick it is in -- `entities` ticked with it,
/// none of them woken by grass.
pub fn tick(simulation: &mut Simulation, arena: &mut BitmapArena, entities: &mut Entities, seed: u64) -> TickReport<GrassCounts> {
    simulation.tick(arena, entities, seed, rule)
}

/// The rule, on one superchunk's turn: every cell of grass where grass
/// grows ([`grows_at`]) chosen with the chances of spreading and of
/// decay together, in Morton order; each draws a neighbour, and whether
/// it tries to spread or to decay, and queues the writes if the
/// neighbour lets it.
pub fn rule(turn: &mut Turn, samples: &mut Vec<CellIndex>) -> GrassCounts {
    if !grows_in(turn.superchunk()) {
        return GrassCounts::default();
    }
    turn.sample(GRASS, SPREAD_CHANCE + DECAY_CHANCE, samples);
    samples.retain(|&cell| grows_at(cell));
    let sampled = samples.len();
    let spread_share = SPREAD_CHANCE / (SPREAD_CHANCE + DECAY_CHANCE);
    let (mut spreads, mut decays) = (0, 0);
    for &cell in samples.iter() {
        let (dx, dy) = NEIGHBOURS[turn.random().below(NEIGHBOURS.len() as u64) as usize];
        let spreading = turn.random().unit() <= spread_share;
        // Stepped on the Morton index itself: no cartesian coordinates.
        let Some(neighbour) = cell.offset(dx, dy) else {
            continue;
        };
        if spreading {
            if turn.holds(DIRT, neighbour) == Ok(true) {
                turn.queue(GRASS, Write::cell(neighbour, WriteOp::Set));
                turn.queue(DIRT, Write::cell(neighbour, WriteOp::Unset));
                spreads += 1;
            }
        } else if turn.holds(GRASS, neighbour) == Ok(true) {
            turn.queue(GRASS, Write::cell(cell, WriteOp::Unset));
            turn.queue(DIRT, Write::cell(cell, WriteOp::Set));
            decays += 1;
        }
    }
    GrassCounts { sampled, spreads, decays }
}
