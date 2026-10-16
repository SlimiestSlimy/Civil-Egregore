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
use entity_manager::{Attribute, WorldEntities, EntityType, Header};
use actions::{cells, entities};
use simulation::{Compare, Simulation, Turn};
use utilities::commands::GivenCommand;

/// Superchunks along the world's side.
pub const WORLD_SIDE_SUPERCHUNKS: &str = "side";
/// Of the cells along a border, one in so many has an entity of the line.
pub const EVERY_SO_MANY_CELLS: &str = "every";
/// Entities only stepping, a superchunk.
pub const STEPPING_A_SUPERCHUNK: &str = "stepping";
/// Ticks run.
pub const TICKS: &str = "ticks";
/// Threads of the second run, the first being on one: 0 for all there are.
pub const THREADS_OF_SECOND_RUN: &str = "threads";

/// Whether those of a line step over their border and back instead
/// of flipping the cell beyond it: 1 if so.
pub const CROSSING: &str = "crossing";

/// The layer flipped.
const STONE_LAYER: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
const WALKER_TYPE: EntityType = EntityType::from_number(40);
/// What one of a line counts.
const COUNTED_ATTRIBUTE: Attribute<u64> = Attribute::new(41);
/// Cells from the border the lines stand when moved away from it.
const AWAY_FROM_BORDER: u32 = 100;

/// The cell `(x, y)` cells from the world's top left.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A world of `world_side_superchunks` by `world_side_superchunks` superchunks: a line along each border
/// running north to south, `cells_away` cells west of it, one entity every
/// `every_so_many_cells` cells, on either side of the border by turns; and
/// `stepping_a_superchunk` entities a superchunk standing apart from them.
fn world(world_side_superchunks: u32, every_so_many_cells: u32, stepping_a_superchunk: u32, cells_away: u32) -> (BitmapArena, WorldEntities) {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (0..world_side_superchunks).flat_map(|y| (0..world_side_superchunks).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(10 + x, 10 + y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
        }
    }
    let mut entity_store = WorldEntities::new();
    entity_store.keep_superchunks(&bitmap_arena.superchunk_indices());
    for border_index in 1..world_side_superchunks {
        for (line_index, y) in (0..world_side_superchunks * SUPERCHUNK_SIDE_CELLS).step_by(every_so_many_cells as usize).enumerate() {
            // West of the border and east of it by turns, so that each superchunk waits on the other all the way down.
            let x = border_index * SUPERCHUNK_SIDE_CELLS - cells_away - if line_index % 2 == 0 { 1 } else { 3 };
            entity_store.queue_put(Header { entity_type: WALKER_TYPE, stood_cell: cell(x + if line_index % 2 == 0 { 0 } else { 3 }, y), wake: 0 }, &[entity_manager::AttributeBlock::holding_value(COUNTED_ATTRIBUTE, 0)]);
        }
    }
    for (x, y) in (0..world_side_superchunks).flat_map(|y| (0..world_side_superchunks).map(move |x| (x, y))) {
        for stepper_index in 0..stepping_a_superchunk {
            let stepper_cell = cell(x * SUPERCHUNK_SIDE_CELLS + 300 + stepper_index % 400, y * SUPERCHUNK_SIDE_CELLS + 4 + 2 * (stepper_index / 400));
            entity_store.queue_put(Header { entity_type: WALKER_TYPE, stood_cell: stepper_cell, wake: 0 }, &[]);
        }
    }
    entity_store.apply();
    (bitmap_arena, entity_store)
}

/// One superchunk's turn: one of a line flips the cell beside it --
/// east if it stands west of its border, west if east -- and counts
/// that on itself, under the compare of the cell; all wake next tick
/// where they stand. If `crossing`, it steps to that cell instead.
fn turn_of(turn: &mut Turn, crossing: bool) -> u64 {
    let tick_now = turn.current_tick();
    for entity in turn.woken_entities() {
        let header = entity.header;
        turn.seeing_to(header.stood_cell);
        if let Some(count_so_far) = entity.attribute(COUNTED_ATTRIBUTE) {
            let walker_cartesian = header.stood_cell.cartesian();
            // Those put west of their border have an odd x less the border's, those east an even one: the world's cells begin even.
            let cell_beside: CellIndex = CellCartesian { x: if walker_cartesian.x % 2 == 1 { walker_cartesian.x + 1 } else { walker_cartesian.x - 1 }, y: walker_cartesian.y }.into();
            if crossing {
                entities::step(turn, &header, cell_beside, tick_now + 1);
                continue;
            }
            let stone_seen = u32::from(turn.layer_holds(STONE_LAYER, cell_beside) == Ok(true));
            turn.compare_from_here(Compare::Cell { layer_type: STONE_LAYER, target_cell: cell_beside, seen_value: stone_seen });
            if stone_seen == 1 { cells::clear(turn, STONE_LAYER, cell_beside) } else { cells::set(turn, STONE_LAYER, cell_beside) }
            entities::set_attribute_of(turn, &header, COUNTED_ATTRIBUTE, Some(count_so_far), count_so_far + 1);
            turn.compare_end();
        }
        entities::step(turn, &header, header.stood_cell, tick_now + 1);
    }
    0
}

/// Runs the probe: a line of CSV for the lines over the border and
/// one for them away from it, on one thread and on every thread.
pub fn run_probe(given_parameters: &GivenCommand) -> Result<(), String> {
    let (world_side_superchunks, every_so_many_cells, stepping_a_superchunk, ticks): (u32, u32, u32, u64) = (given_parameters.argument_number(WORLD_SIDE_SUPERCHUNKS)?, given_parameters.argument_number(EVERY_SO_MANY_CELLS)?, given_parameters.argument_number(STEPPING_A_SUPERCHUNK)?, given_parameters.argument_number(TICKS)?);
    println!("lines,threads,acts over a border a tick,applying us a tick,computing us a tick,compares held a tick,compares not a tick");
    let (threads_asked, crossing): (usize, u32) = (given_parameters.argument_number(THREADS_OF_SECOND_RUN)?, given_parameters.argument_number(CROSSING)?);
    for thread_count in [1, if threads_asked == 0 { simulation::threads_for((world_side_superchunks * world_side_superchunks) as usize) } else { threads_asked }] {
        for (placement_name, cells_away) in [("away from the border", AWAY_FROM_BORDER), ("over the border", 0)] {
            let (mut bitmap_arena, mut entity_store) = world(world_side_superchunks, every_so_many_cells, stepping_a_superchunk, cells_away);
            let mut timed_simulation = Simulation::new(thread_count);
            let (mut acts_over_border, mut applying_time, mut computing_time, mut compares_counted) = (0, std::time::Duration::ZERO, std::time::Duration::ZERO, (0, 0));
            for tick in 0..ticks + 20 {
                let report = timed_simulation.tick(&mut bitmap_arena, &mut entity_store, 1, |turn: &mut Turn| turn_of(turn, crossing == 1));
                // The first ticks grow the queues: not counted.
                if tick >= 20 {
                    (acts_over_border, applying_time, computing_time) = (acts_over_border + report.acts_over_a_border, applying_time + report.applying_time, computing_time + report.computing_time);
                    compares_counted = (compares_counted.0 + report.instructions_compared.0, compares_counted.1 + report.instructions_compared.1);
                }
            }
            println!("{placement_name},{thread_count},{},{:.1},{:.1},{},{}", acts_over_border / ticks, applying_time.as_secs_f64() * 1e6 / ticks as f64, computing_time.as_secs_f64() * 1e6 / ticks as f64, compares_counted.0 / ticks, compares_counted.1 / ticks);
        }
    }
    Ok(())
}
