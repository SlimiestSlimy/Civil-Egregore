//! What an act over a border costs the second phase: lines of
//! entities along every border running north to south, each flipping
//! the cell east of it and counting that on itself under the one
//! compare, every tick -- over the border, or the same lines moved
//! away from it -- with others only stepping inside each superchunk.
//! Or, `crossing`, each of a line stepping to that cell and back the
//! next tick: over the border every tick, with no compare of its own.

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, Entities, EntityType, Header};
use actions::{cells, entities};
use simulation::{Compare, Simulation, Turn};
use utilities::commands::Given;

/// Superchunks along the world's side.
pub const SIDE: &str = "side";
/// Of the cells along a border, one in so many has an entity of the line.
pub const EVERY: &str = "every";
/// Entities only stepping, a superchunk.
pub const STEPPING: &str = "stepping";
/// Ticks run.
pub const TICKS: &str = "ticks";
/// Threads of the second run, the first being on one: 0 for all there are.
pub const THREADS: &str = "threads";

/// Whether those of a line step over their border and back instead
/// of flipping the cell beyond it: 1 if so.
pub const CROSSING: &str = "crossing";

/// The layer flipped.
const STONE: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
const WALKER: EntityType = EntityType::of(40);
/// What one of a line counts.
const COUNTED: Attribute<u64> = Attribute::new(41);
/// Cells from the border the lines stand when moved away from it.
const AWAY: u32 = 100;

/// The cell `(x, y)` cells from the world's top left.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A world of `side` by `side` superchunks: a line along each border
/// running north to south, `away` cells west of it, one entity every
/// `every` cells, on either side of the border by turns; and
/// `stepping` entities a superchunk standing apart from them.
fn world(side: u32, every: u32, stepping: u32, away: u32) -> (BitmapArena, Entities) {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(10 + x, 10 + y).chunks() {
            arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
        }
    }
    let mut entities = Entities::new();
    entities.align(&arena.superchunk_indices());
    for border in 1..side {
        for (nth, y) in (0..side * SUPERCHUNK_SIDE_CELLS).step_by(every as usize).enumerate() {
            // West of the border and east of it by turns, so that each superchunk waits on the other all the way down.
            let x = border * SUPERCHUNK_SIDE_CELLS - away - if nth % 2 == 0 { 1 } else { 3 };
            entities.queue_put(Header { kind: WALKER, at: cell(x + if nth % 2 == 0 { 0 } else { 3 }, y), wake: 0 }, &[entity_manager::AttributeBlock::holding(COUNTED, 0)]);
        }
    }
    for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
        for nth in 0..stepping {
            let at = cell(x * SUPERCHUNK_SIDE_CELLS + 300 + nth % 400, y * SUPERCHUNK_SIDE_CELLS + 4 + 2 * (nth / 400));
            entities.queue_put(Header { kind: WALKER, at, wake: 0 }, &[]);
        }
    }
    entities.apply();
    (arena, entities)
}

/// One superchunk's turn: one of a line flips the cell beside it --
/// east if it stands west of its border, west if east -- and counts
/// that on itself, under the compare of the cell; all wake next tick
/// where they stand. If `crossing`, it steps to that cell instead.
fn turn_of(turn: &mut Turn, crossing: bool) -> u64 {
    let now = turn.now();
    for entity in turn.woken() {
        let header = entity.header;
        turn.seeing_to(header.at);
        if let Some(counted) = entity.attribute(COUNTED) {
            let at = header.at.cartesian();
            // Those put west of their border have an odd x less the border's, those east an even one: the world's cells begin even.
            let beside: CellIndex = CellCartesian { x: if at.x % 2 == 1 { at.x + 1 } else { at.x - 1 }, y: at.y }.into();
            if crossing {
                entities::step(turn, &header, beside, now + 1);
                continue;
            }
            let seen = u32::from(turn.holds(STONE, beside) == Ok(true));
            turn.compare_from_here(Compare::Cell { layer_type: STONE, at: beside, seen: seen as u16 });
            if seen == 1 { cells::clear(turn, STONE, beside) } else { cells::set(turn, STONE, beside) }
            entities::set_attribute_of(turn, &header, COUNTED, Some(counted), counted + 1);
            turn.compare_end();
        }
        entities::step(turn, &header, header.at, now + 1);
    }
    0
}

/// Runs the probe: a line of CSV for the lines over the border and
/// one for them away from it, on one thread and on every thread.
pub fn run(given: &Given) -> Result<(), String> {
    let (side, every, stepping, ticks): (u32, u32, u32, u64) = (given.number(SIDE)?, given.number(EVERY)?, given.number(STEPPING)?, given.number(TICKS)?);
    println!("lines,threads,acts over a border a tick,applying us a tick,computing us a tick,compares held a tick,compares not a tick");
    let (threads, crossing): (usize, u32) = (given.number(THREADS)?, given.number(CROSSING)?);
    for threads in [1, if threads == 0 { simulation::threads_for((side * side) as usize) } else { threads }] {
        for (name, away) in [("away from the border", AWAY), ("over the border", 0)] {
            let (mut arena, mut entities) = world(side, every, stepping, away);
            let mut simulation = Simulation::new(threads);
            let (mut acts, mut applying, mut computing, mut compared) = (0, std::time::Duration::ZERO, std::time::Duration::ZERO, (0, 0));
            for tick in 0..ticks + 20 {
                let report = simulation.tick(&mut arena, &mut entities, 1, |turn: &mut Turn, _: &mut Vec<CellIndex>| turn_of(turn, crossing == 1));
                // The first ticks grow the queues: not counted.
                if tick >= 20 {
                    (acts, applying, computing) = (acts + report.acts_over_a_border, applying + report.applying, computing + report.computing);
                    compared = (compared.0 + report.instructions_compared.0, compared.1 + report.instructions_compared.1);
                }
            }
            println!("{name},{threads},{},{:.1},{:.1},{},{}", acts / ticks, applying.as_secs_f64() * 1e6 / ticks as f64, computing.as_secs_f64() * 1e6 / ticks as f64, compared.0 / ticks, compared.1 / ticks);
        }
    }
    Ok(())
}
