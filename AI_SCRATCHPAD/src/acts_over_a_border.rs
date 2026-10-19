//! What an act over a border costs the second phase: lines of
//! entities along every border running north to south, each flipping
//! the cell east of it and counting that on itself under the one
//! compare, every tick -- over the border, or the same lines moved
//! away from it -- with others only stepping inside each superchunk.
//! Or, `crossing`, each of a line stepping to that cell and back the
//! next tick: over the border every tick, with no compare of its own.

// TEMPORARY: reaches the server's internals -- `server::internals_for_root_tests`
// and the world's handles to them -- until the world has reads of its
// own and this is rewritten against them (AI.txt, LATER).

use server::internals_for_root_tests::{BitmapArena, BucketKey};
use server::internals_for_root_tests::{LayerCodec, LayerType};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use server::internals_for_root_tests::{Attribute, WorldEntities, EntityType, Header};
use server::simulation::simulation_api::{cell_reads_and_writes, compare_and_write, entity_reads_and_writes};
use server::internals_for_root_tests::{Compare, Simulation, SimulationTickState};
use crate::ProbeParameters;
use protocol::type_registry::{Registration, TypeRegistry};

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

/// The probe's types, registered in a registry of its own -- never a
/// number by hand.
#[derive(Clone, Copy)]
struct ProbeTypes {
    /// The layer flipped.
    stone: LayerType,
    /// The entities' type.
    walker: EntityType,
    /// What one of a line counts.
    counted: Attribute<u64>,
}

impl ProbeTypes {
    /// Every one registered.
    fn registered() -> Self {
        let mut type_registry = TypeRegistry::for_new_world();
        let mut register = |registration: Registration| type_registry.register(registration).expect("a type of a name no other has");
        let stone = register(Registration::layer("stone")).layer_type().expect("a layer");
        let walker = register(Registration::entity("walker", None)).entity_type().expect("a type of entity");
        let counted = Attribute::registered(&register(Attribute::<u64>::registration("counted"))).expect("a word's attribute");
        Self { stone, walker, counted }
    }
}
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
fn world(probe_types: ProbeTypes, world_side_superchunks: u32, every_so_many_cells: u32, stepping_a_superchunk: u32, cells_away: u32) -> (BitmapArena, WorldEntities) {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (0..world_side_superchunks).flat_map(|y| (0..world_side_superchunks).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(10 + x, 10 + y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: probe_types.stone, chunk }, None, &mut layer_codec);
        }
    }
    let mut entity_store = WorldEntities::new();
    entity_store.keep_superchunks(&bitmap_arena.superchunk_indices());
    for border_index in 1..world_side_superchunks {
        for (line_index, y) in (0..world_side_superchunks * SUPERCHUNK_SIDE_CELLS).step_by(every_so_many_cells as usize).enumerate() {
            // West of the border and east of it by turns, so that each superchunk waits on the other all the way down.
            let x = border_index * SUPERCHUNK_SIDE_CELLS - cells_away - if line_index % 2 == 0 { 1 } else { 3 };
            entity_store.queue_add(Header { entity_type: probe_types.walker, stood_cell: cell(x + if line_index % 2 == 0 { 0 } else { 3 }, y), wake: 0 }, &[server::internals_for_root_tests::AttributeBlock::holding_value(probe_types.counted, 0)]);
        }
    }
    for (x, y) in (0..world_side_superchunks).flat_map(|y| (0..world_side_superchunks).map(move |x| (x, y))) {
        for stepper_index in 0..stepping_a_superchunk {
            let stepper_cell = cell(x * SUPERCHUNK_SIDE_CELLS + 300 + stepper_index % 400, y * SUPERCHUNK_SIDE_CELLS + 4 + 2 * (stepper_index / 400));
            entity_store.queue_add(Header { entity_type: probe_types.walker, stood_cell: stepper_cell, wake: 0 }, &[]);
        }
    }
    entity_store.apply();
    (bitmap_arena, entity_store)
}

/// One superchunk's turn: one of a line flips the cell beside it --
/// east if it stands west of its border, west if east -- and counts
/// that on itself, under the compare of the cell; all wake next tick
/// where they stand. If `crossing`, it steps to that cell instead.
fn turn_of(probe_types: ProbeTypes, tick_state: &mut SimulationTickState, crossing: bool) -> u64 {
    let tick_now = tick_state.current_tick();
    for entity in tick_state.woken_entities() {
        let header = entity.header;
        tick_state.seeing_to(header.stood_cell);
        if let Some(count_so_far) = entity.attribute(probe_types.counted) {
            let walker_cartesian = header.stood_cell.cartesian();
            // Those put west of their border have an odd x less the border's, those east an even one: the world's cells begin even.
            let cell_beside: CellIndex = CellCartesian { x: if walker_cartesian.x % 2 == 1 { walker_cartesian.x + 1 } else { walker_cartesian.x - 1 }, y: walker_cartesian.y }.into();
            if crossing {
                entity_reads_and_writes::step(tick_state, &header, cell_beside, tick_now + 1);
                continue;
            }
            let stone_seen = u32::from(tick_state.layer_holds(probe_types.stone, cell_beside) == Ok(true));
            let mut stone_as_seen = compare_and_write::open_compare(tick_state, Compare::Cell { layer_type: probe_types.stone, target_cell: cell_beside, seen_value: stone_seen });
            if stone_seen == 1 { cell_reads_and_writes::clear(&mut stone_as_seen, probe_types.stone, cell_beside) } else { cell_reads_and_writes::set(&mut stone_as_seen, probe_types.stone, cell_beside) }
            entity_reads_and_writes::set_attribute_of(&mut stone_as_seen, &header, probe_types.counted, Some(count_so_far), count_so_far + 1);
        }
        entity_reads_and_writes::step(tick_state, &header, header.stood_cell, tick_now + 1);
    }
    0
}

/// Runs the probe: a line of CSV for the lines over the border and
/// one for them away from it, on one thread and on every thread.
pub fn run_probe(given_parameters: &ProbeParameters) -> Result<(), String> {
    let (world_side_superchunks, every_so_many_cells, stepping_a_superchunk, ticks): (u32, u32, u32, u64) = (given_parameters.argument_number(WORLD_SIDE_SUPERCHUNKS)?, given_parameters.argument_number(EVERY_SO_MANY_CELLS)?, given_parameters.argument_number(STEPPING_A_SUPERCHUNK)?, given_parameters.argument_number(TICKS)?);
    println!("lines,threads,acts over a border a tick,applying us a tick,computing us a tick,compares held a tick,compares not a tick");
    let (threads_asked, crossing): (usize, u32) = (given_parameters.argument_number(THREADS_OF_SECOND_RUN)?, given_parameters.argument_number(CROSSING)?);
    let probe_types = ProbeTypes::registered();
    for thread_count in [1, if threads_asked == 0 { server::internals_for_root_tests::threads_for((world_side_superchunks * world_side_superchunks) as usize) } else { threads_asked }] {
        for (placement_name, cells_away) in [("away from the border", AWAY_FROM_BORDER), ("over the border", 0)] {
            let (mut bitmap_arena, mut entity_store) = world(probe_types, world_side_superchunks, every_so_many_cells, stepping_a_superchunk, cells_away);
            let mut timed_simulation = Simulation::new(thread_count);
            let (mut acts_over_border, mut applying_time, mut computing_time, mut compares_counted) = (0, std::time::Duration::ZERO, std::time::Duration::ZERO, (0, 0));
            for tick in 0..ticks + 20 {
                let report = timed_simulation.tick(&mut bitmap_arena, &mut entity_store, 1, |tick_state: &mut SimulationTickState| turn_of(probe_types, tick_state, crossing == 1));
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
