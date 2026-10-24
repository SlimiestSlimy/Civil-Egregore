//! An act over a border: what an author queues for more than one
//! superchunk is applied in one piece -- what is under a compare
//! whole or not at all -- in the one order of the world, however the superchunks wait on one
//! another (`docs/simulation.md`, "An act over a border").
//!
//! `cargo test -p server --lib acts_over_a_border`

use crate::arenas::{cell, walker_base_data, world};
use crate::test_types::TestTypes;
use crate::bitplane_manager::BitmapArena;
use utilities::coordinates::{CellIndex, SUPERCHUNK_SIDE_CELLS};
use crate::entity_manager::WorldEntities;
use protocol::server_types::entity::NEVER_WAKES;
use protocol::server_types::attributes::AttributeBlock;
use crate::simulation::{Compare, Simulation, SimulationTickState};
use std::collections::BTreeSet;
use utilities::rng::RandomSource;

/// An entity beside a border -- any of the four that meet at the
/// corner of four superchunks, on either side of it, anywhere along
/// it: drawn from the seed -- makes another over it and counts that
/// on itself, under the one compare of the cell being free: free,
/// both happen; taken, neither -- and it sleeps either way.
#[test]
fn an_act_over_a_border_happens_whole_or_not_at_all() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut seeded_rng = RandomSource::new(seed);
    for is_taken in [false, true, false, true] {
        let (mut bitmap_arena, mut entity_store) = world(types, 2);
        // Along the border, and the two cells either side of it: which the maker stands on, and whether the border runs north to south.
        let (along_border, maker_side, north_to_south) = (seeded_rng.draw_below(u64::from(2 * SUPERCHUNK_SIDE_CELLS)) as u32, seeded_rng.draw_below(2) as u32, seeded_rng.draw_below(2) == 0);
        let cell_across = |border_side: u32| if north_to_south { cell(SUPERCHUNK_SIDE_CELLS - 1 + border_side, along_border) } else { cell(along_border, SUPERCHUNK_SIDE_CELLS - 1 + border_side) };
        let (maker_base_data, beside_cell) = (walker_base_data(types, cell_across(maker_side), 0), cell_across(1 - maker_side));
        entity_store.queue_add(maker_base_data, &[]);
        if is_taken {
            entity_store.queue_add(walker_base_data(types, beside_cell, NEVER_WAKES), &[]);
        }
        entity_store.apply();
        let report = Simulation::new(2).tick(&mut bitmap_arena, &mut entity_store, seeded_rng.next_draw(), |tick_state: &mut SimulationTickState| {
            for entity in tick_state.woken_entities() {
                tick_state.seeing_to(entity.base_data.stood_cell);
                let mut entity_made = crate::simulation::simulation_api::entity_reads_and_writes::begin_making_entity(tick_state, types.walker, beside_cell, NEVER_WAKES, &[AttributeBlock::holding_value(types.entity_number, 5)]);
                crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(&mut entity_made, &entity.base_data, types.woken_count, None, 1);
                drop(entity_made);
                crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &entity.base_data, entity.base_data.stood_cell, NEVER_WAKES);
            }
            0
        });
        assert_eq!(report.acts_over_a_border, 1, "taken: {is_taken}");
        let (made_tag, maker_entity) = (entity_store.get(beside_cell).and_then(|made_entity| made_entity.attribute(types.entity_number)), entity_store.get(maker_base_data.stood_cell).expect("where it stood"));
        assert_eq!((made_tag, maker_entity.attribute(types.woken_count), maker_entity.base_data.wake), if is_taken { (None, None, NEVER_WAKES) } else { (Some(5), Some(1), NEVER_WAKES) }, "taken: {is_taken}");
    }
}

/// The most cells each way from the corner four superchunks meet at
/// that the crowd stands on.
const CROWD_REACH: u32 = 12;
/// The most cells along each border from the corner that a line of
/// the crowd stands on.
const LINE: u32 = 200;
/// Ticks the crowd is run.
const TICKS: u64 = 31;

/// Where the crowd stands, drawn from `seed`: about half the cells of
/// a square about the corner of four superchunks, and of two lines
/// along the borders from it, a cell each side -- the square and the
/// lines as large as drawn, never less than half the most.
fn crowd_cartesian(seed: u64) -> BTreeSet<(u32, u32)> {
    let (mut crowd_rng, corner_coordinate) = (RandomSource::new(seed), SUPERCHUNK_SIDE_CELLS);
    let (square_half_side, line) = (CROWD_REACH / 2 + crowd_rng.draw_below(u64::from(CROWD_REACH / 2) + 1) as u32, LINE / 2 + crowd_rng.draw_below(u64::from(LINE / 2) + 1) as u32);
    let square_cells = (corner_coordinate - square_half_side..corner_coordinate + square_half_side).flat_map(|y| (corner_coordinate - square_half_side..corner_coordinate + square_half_side).map(move |x| (x, y)));
    let line_cells = (corner_coordinate..corner_coordinate + line).flat_map(|along_line| [(corner_coordinate - 1, along_line), (corner_coordinate, along_line), (along_line, corner_coordinate - 1), (along_line, corner_coordinate)]);
    square_cells.chain(line_cells).collect::<BTreeSet<_>>().into_iter().filter(|_| crowd_rng.draw_below(2) == 0).collect()
}

/// One superchunk's turn of the crowd: each, one tick in three drawn
/// from `seed` by where it stands, flips the stone east of it, south of it, and
/// south-east past that, as it saw them -- the third only if the
/// second is still as seen, all only if the first is -- and counts on
/// itself, under the first compare too. A cell is three entities' to
/// flip, so of those acting in a tick the first in the world's order
/// has it and the others are refused: in up to four superchunks at
/// once.
fn flip_what_is_about(types: TestTypes, tick_state: &mut SimulationTickState, seed: u64) -> u64 {
    let current_tick = tick_state.current_tick();
    for entity in tick_state.woken_entities() {
        let (base_data, stood_cartesian) = (entity.base_data, entity.base_data.stood_cell.cartesian());
        tick_state.seeing_to(base_data.stood_cell);
        if RandomSource::for_stream(seed ^ current_tick, u64::from(stood_cartesian.y) << 32 | u64::from(stood_cartesian.x)).draw_below(3) != 0 {
            crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &base_data, base_data.stood_cell, current_tick + 1);
            continue;
        }
        let seen_at = |tick_state: &SimulationTickState, x: u32, y: u32| (CellIndex::from(utilities::coordinates::CellCartesian { x, y }), u32::from(tick_state.layer_holds(types.stone, utilities::coordinates::CellCartesian { x, y }.into()) == Ok(true)));
        let (east_seen, south_seen, past_seen) = (seen_at(tick_state, stood_cartesian.x + 1, stood_cartesian.y), seen_at(tick_state, stood_cartesian.x, stood_cartesian.y + 1), seen_at(tick_state, stood_cartesian.x + 1, stood_cartesian.y + 2));
        let mut east_as_seen = crate::simulation::simulation_api::compare_and_write::open_compare(tick_state, Compare::Cell { layer_type: types.stone, target_cell: east_seen.0, seen_value: east_seen.1 });
        crate::simulation::simulation_api::compare_and_write::write(&mut east_as_seen, Compare::Cell { layer_type: types.stone, target_cell: south_seen.0, seen_value: south_seen.1 }, types.stone, past_seen.0, past_seen.1, 1 - past_seen.1, None);
        crate::simulation::simulation_api::cell_reads_and_writes::seen_becoming(&mut east_as_seen, types.stone, south_seen.0, south_seen.1, 1 - south_seen.1, None);
        crate::simulation::simulation_api::cell_reads_and_writes::seen_becoming(&mut east_as_seen, types.stone, east_seen.0, east_seen.1, 1 - east_seen.1, None);
        let counted_wakes = entity.attribute(types.woken_count);
        crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(&mut east_as_seen, &base_data, types.woken_count, counted_wakes, counted_wakes.unwrap_or(0) + 1);
        drop(east_as_seen);
        crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &base_data, base_data.stood_cell, current_tick + 1);
    }
    0
}

/// What came of a crowd: the cells of stone about it, and each of it
/// by its cell with what it counted.
type CrowdOutcome = (Vec<(u32, u32)>, Vec<(u64, Option<u64>)>);

/// What came of the crowd: the cells of stone about it, and what each
/// of it counted.
fn crowd_outcome(types: TestTypes, bitmap_arena: &BitmapArena, entity_store: &WorldEntities) -> CrowdOutcome {
    let about_range = SUPERCHUNK_SIDE_CELLS - CROWD_REACH - 2..SUPERCHUNK_SIDE_CELLS + LINE + 4;
    let stone_cells = about_range.clone().flat_map(|y| about_range.clone().map(move |x| (x, y))).filter(|&(x, y)| bitmap_arena.layer_holds_cell(types.stone, utilities::coordinates::CellCartesian { x: cell(0, 0).cartesian().x + x, y: cell(0, 0).cartesian().y + y }.into()) == Ok(true)).collect();
    let mut counted_by_cell: Vec<_> = entity_store.iter().map(|entity| (entity.base_data.stood_cell.0, entity.attribute(types.woken_count))).collect();
    counted_by_cell.sort_unstable();
    (stone_cells, counted_by_cell)
}

/// The worst there is for superchunks waiting on one another -- a
/// crowd drawn from the seed about the corner of four and along their
/// borders, those of it by a border acting over it, up to three
/// contending for each cell -- ends, and the same on one thread as on
/// any number: every cell of stone and every count.
#[test]
fn a_crowd_acting_over_every_border_comes_to_the_same_on_any_threads() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut first_outcome = None;
    for thread_count in [1, 2, 3, 4, 16] {
        let (mut bitmap_arena, mut entity_store) = world(types, 2);
        let crowd_cells = crowd_cartesian(seed);
        for &(x, y) in &crowd_cells {
            entity_store.queue_add(walker_base_data(types, cell(x, y), 0), &[]);
        }
        entity_store.apply();
        let mut world_simulation = Simulation::new(thread_count);
        let (mut over_a_border, mut refused_count) = (0, 0);
        for _ in 0..TICKS {
            let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| flip_what_is_about(types, tick_state, seed));
            over_a_border += report.acts_over_a_border;
            refused_count += report.instructions_compared.1;
        }
        // Of those by a border a third act over it every tick: more acts than there are of the crowd, whatever was drawn.
        assert!(over_a_border > crowd_cells.len() as u64 && refused_count > 0, "seed {seed:#x}: {over_a_border} acts over a border by a crowd of {}, {refused_count} compares not holding", crowd_cells.len());
        let thread_outcome = (crowd_outcome(types, &bitmap_arena, &entity_store), over_a_border, refused_count);
        // Stone is left, and most of the crowd counted an act of its own: those of the lines too, all of whose acts are over a border.
        let counting_members = thread_outcome.0 .1.iter().filter(|counted_entry| counted_entry.1.is_some()).count();
        assert!(!thread_outcome.0 .0.is_empty() && counting_members > thread_outcome.0 .1.len() / 2, "seed {seed:#x}: {} cells of stone, {counting_members} of {} counting; {over_a_border} acts, {refused_count} refused", thread_outcome.0 .0.len(), thread_outcome.0 .1.len());
        match &first_outcome {
            None => first_outcome = Some(thread_outcome),
            Some(earlier_outcome) => assert!(*earlier_outcome == thread_outcome, "seed {seed:#x}, on {thread_count} threads: {} cells of stone and {} acts, on one {} and {}", thread_outcome.0 .0.len(), thread_outcome.1, earlier_outcome.0 .0.len(), earlier_outcome.1),
        }
    }
}
