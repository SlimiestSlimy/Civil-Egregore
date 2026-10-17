//! Compare-and-write: a write applied only if what it is held against
//! is as its rule saw it -- the cell written, another cell, an entity
//! -- the cell made to decay and be eaten in one tick, on purpose, and
//! all of it the same on any number of threads.

use crate::arenas::{cell, walker_header, world, STONE_LAYER};
use crate::walkers::{cells_drawn};
use bitplane_manager::{BitmapArena, Write, WriteOp};
use utilities::coordinates::CellIndex;
use entity_manager::{Attribute, AttributeBlock};
use simulation::{Compare, Simulation, SimulationTickState};
use utilities::rng::RandomSource;

/// An attribute an entity has once it has eaten.
const FED_ATTRIBUTE: Attribute<u64> = Attribute::new(43);
/// The place cells decayed are counted at.
const DECAYED_COUNT_PLACE: u32 = 3;
/// The place meals are counted at.
const EATEN_COUNT_PLACE: u32 = 4;

/// Sets `cells` of the stone, between two ticks.
fn set(bitmap_arena: &mut BitmapArena, cells: &[CellIndex]) {
    cells.iter().for_each(|&set_cell| bitmap_arena.queue_write(STONE_LAYER, Write::cell(set_cell, WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().cells_changed, cells.len() as u64);
}

/// Two writes on one cell that both saw it clear: the first applied
/// sets it, the second is refused. A write that says it saw what the
/// cell did not hold is refused, and nothing happens. Each counts only
/// if applied, under the number its rule's counts were given.
#[test]
fn a_write_is_applied_only_where_the_cell_is_as_it_was_seen() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    let (written_twice_cell, never_set, cleared_cell) = (cell(5, 5), cell(700, 9), cell(1023, 1023));
    set(&mut bitmap_arena, &[cleared_cell]);
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        tick_state.count_under(10);
        simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, written_twice_cell, 0, 1, Some(DECAYED_COUNT_PLACE as usize));
        simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, written_twice_cell, 0, 1, Some(DECAYED_COUNT_PLACE as usize));
        simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, never_set, 1, 0, Some(EATEN_COUNT_PLACE as usize));
        simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, cleared_cell, 1, 0, None);
        0
    });
    let report_writes = report.writes_applied;
    assert_eq!((report_writes.writes, report_writes.cells_changed, report_writes.writes_refused, report_writes.missed), (4, 2, 2, 0));
    assert_eq!((bitmap_arena.layer_holds_cell(STONE_LAYER, written_twice_cell), bitmap_arena.layer_holds_cell(STONE_LAYER, never_set), bitmap_arena.layer_holds_cell(STONE_LAYER, cleared_cell)), (Ok(true), Ok(false), Ok(false)));
    assert_eq!((report.counted_when_applied[10 + DECAYED_COUNT_PLACE as usize], report.counted_when_applied[10 + EATEN_COUNT_PLACE as usize]), (1, 0));
    assert_eq!(report.counted_when_applied.iter().sum::<u64>(), 1);
}

/// A cell made to decay and be eaten in one tick, on purpose: two
/// eaters, each on a set cell. Each queues a sleep that stands
/// whatever comes; then, held against its cell being set still, what
/// it comes to having eaten; then its cell cleared, the meal counted;
/// then a mark set on the cell beside it, held against the eater
/// itself being fed -- an entity written on a compare of a cell, a
/// cell on a compare of an entity. The first one's cell is cleared by
/// a write queued before the meals. That cell is cleared once and the
/// decay counted; every write of its eater's meal is refused, one
/// after another -- not fed, not counted, no mark, asleep only a tick
/// -- and the other's are all applied.
#[test]
fn a_cell_decayed_and_eaten_in_one_tick_is_a_meal_refused_write_by_write() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    let (decaying_cell, eaten_cell) = (cell(300, 300), cell(301, 400));
    set(&mut bitmap_arena, &[decaying_cell, eaten_cell]);
    entity_store.queue_add(walker_header(decaying_cell, 0), &[]);
    entity_store.queue_add(walker_header(eaten_cell, 0), &[]);
    entity_store.apply();
    let mark_cell_of = |eater_cell: CellIndex| eater_cell.at_cartesian_offset(1, 0).expect("in the world");
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, decaying_cell, 1, 0, Some(DECAYED_COUNT_PLACE as usize));
        for eater_entity in tick_state.woken_entities() {
            let (current_tick, header) = (tick_state.current_tick(), eater_entity.header);
            // Fed if the cell still holds, asleep a tick if it does not: one of the two, the eater written once.
            simulation::simulation_api::entity_reads_and_writes::update_entity(&mut simulation::simulation_api::compare_and_write::open_compare(tick_state, Compare::Cell { layer_type: STONE_LAYER, target_cell: header.stood_cell, seen_value: 1 }), &header, entity_manager::Header { wake: current_tick + 100, ..header }, &[AttributeBlock::holding_value(FED_ATTRIBUTE, 1)]);
            simulation::simulation_api::entity_reads_and_writes::step(&mut simulation::simulation_api::compare_and_write::open_compare(tick_state, Compare::Cell { layer_type: STONE_LAYER, target_cell: header.stood_cell, seen_value: 0 }), &header, header.stood_cell, current_tick + 1);
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, header.stood_cell, 1, 0, Some(EATEN_COUNT_PLACE as usize));
            let fed_compare = Compare::attribute(header.stood_cell, FED_ATTRIBUTE.attribute_type(), Some(&[AttributeBlock::holding_value(FED_ATTRIBUTE, 1)]));
            simulation::simulation_api::compare_and_write::write(tick_state, fed_compare, STONE_LAYER, mark_cell_of(header.stood_cell), 0, 1, None);
        }
        0
    });
    assert_eq!((bitmap_arena.layer_holds_cell(STONE_LAYER, decaying_cell), bitmap_arena.layer_holds_cell(STONE_LAYER, eaten_cell)), (Ok(false), Ok(false)));
    assert_eq!((bitmap_arena.layer_holds_cell(STONE_LAYER, mark_cell_of(decaying_cell)), bitmap_arena.layer_holds_cell(STONE_LAYER, mark_cell_of(eaten_cell))), (Ok(false), Ok(true)), "a mark by the one fed alone");
    let report_writes = report.writes_applied;
    assert_eq!((report_writes.writes, report_writes.cells_changed, report_writes.writes_refused), (5, 3, 2), "the meal on the cell decayed and its mark, refused");
    // Every write's compare is an instruction: the decay's holds; the eater fed holds six of its seven (fed, nothing seen of the
    // attribute it sets, itself there to be put to sleep, its meal's cell, its attribute, its mark's cell; not asleep); the other one of four come to (asleep; not fed, its meal's cell, its attribute -- the
    // mark's cell, within that, never come to).
    assert_eq!(report.instructions_compared, (9, 4), "one eater fed, one asleep a tick: each written once");
    assert_eq!((report.counted_when_applied[DECAYED_COUNT_PLACE as usize], report.counted_when_applied[EATEN_COUNT_PLACE as usize]), (1, 1));
    let (hungry_entity, fed_entity) = (entity_store.get(decaying_cell).expect("where it stood"), entity_store.get(eaten_cell).expect("where it stood"));
    assert_eq!((hungry_entity.attribute(FED_ATTRIBUTE), hungry_entity.header.wake), (None, 1), "nothing of its meal happened: asleep a tick");
    assert_eq!((fed_entity.attribute(FED_ATTRIBUTE), fed_entity.header.wake), (Some(1), 100), "its meal whole");
}

/// What a superchunk's turn queues in
/// [`drawn_writes_come_to_the_same_as_applied_in_order`], for the
/// cells of `cells` in it, each as it was `seen`: for each, in order,
/// one to three times, a write that flips it -- held against itself,
/// or against the next cell of its superchunk being as seen. Given to
/// `queue_attempt` as the cell flipped and the cell compared.
fn queue_attempts(seed: u64, cells: &[(CellIndex, bool)], superchunk: utilities::coordinates::SuperchunkIndex, mut queue_attempt: impl FnMut((CellIndex, bool), (CellIndex, bool))) {
    let cells_here: Vec<(CellIndex, bool)> = cells.iter().copied().filter(|(drawn_cell, _)| drawn_cell.superchunk() == superchunk).collect();
    for (cell_number, &this_cell) in cells_here.iter().enumerate() {
        // Drawn from the cell, not the turn: the same whoever asks.
        let mut cell_rng = RandomSource::new(this_cell.0 .0 ^ seed);
        for _ in 0..cell_rng.draw_between(1, 3) {
            match cell_rng.draw_below(2) == 0 || cells_here.len() < 2 {
                true => queue_attempt(this_cell, this_cell),
                false => queue_attempt(this_cell, cells_here[(cell_number + 1) % cells_here.len()]),
            }
        }
    }
}

/// Cells drawn over four superchunks, some set, each written one to
/// three times in a tick, each write held against what the tick found
/// of its own cell or of another: what comes of it is what applying
/// them one after another in the order queued comes to -- a write
/// applied if what it is held against is still as seen, refused
/// otherwise -- cell by cell and count by count, on one thread and on
/// four.
#[test]
fn drawn_writes_come_to_the_same_as_applied_in_order() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    let cells: Vec<(CellIndex, bool)> = cells_drawn(seed, 400, 2048).into_iter().map(|drawn_cell| (drawn_cell, seeded_rng.draw_below(2) == 0)).collect();
    // What applying them in order comes to: the cells, the writes held against their own cell applied, against another, and refused.
    let (mut expected_cells, mut own_applied, mut other_applied, mut refused_count): (std::collections::HashMap<CellIndex, bool>, _, _, _) = (cells.iter().copied().collect(), 0u64, 0u64, 0u64);
    let mut superchunks: Vec<utilities::coordinates::SuperchunkIndex> = cells.iter().map(|(superchunk_cell, _)| superchunk_cell.superchunk()).collect();
    superchunks.sort_unstable();
    superchunks.dedup();
    for &superchunk in &superchunks {
        queue_attempts(seed, &cells, superchunk, |flipped_entry, compared_entry| match (expected_cells[&compared_entry.0] == compared_entry.1 && expected_cells[&flipped_entry.0] == flipped_entry.1, flipped_entry.0 == compared_entry.0) {
            (false, _) => refused_count += 1,
            (true, same_cell) => {
                (own_applied, other_applied) = (own_applied + u64::from(same_cell), other_applied + u64::from(!same_cell));
                expected_cells.insert(flipped_entry.0, !flipped_entry.1);
            }
        });
    }
    assert!(own_applied > 0 && other_applied > 0 && refused_count > 0, "every case drawn");
    for thread_count in [1, 4] {
        let (mut bitmap_arena, mut entity_store) = world(2);
        set(&mut bitmap_arena, &cells.iter().filter(|(_, is_set)| *is_set).map(|(set_cell, _)| *set_cell).collect::<Vec<_>>());
        let report = Simulation::new(thread_count).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
            let superchunk = tick_state.superchunk();
            queue_attempts(seed, &cells, superchunk, |turn_flipped, turn_compared| {
                let held_against = Compare::Cell { layer_type: STONE_LAYER, target_cell: turn_compared.0, seen_value: u32::from(turn_compared.1) };
                simulation::simulation_api::compare_and_write::write(tick_state, held_against, STONE_LAYER, turn_flipped.0, u32::from(turn_flipped.1), u32::from(!turn_flipped.1), Some(usize::from(turn_flipped.0 != turn_compared.0)));
            });
            0
        });
        for (checked_cell, _) in &cells {
            assert_eq!(bitmap_arena.layer_holds_cell(STONE_LAYER, *checked_cell), Ok(expected_cells[checked_cell]), "on {thread_count} threads");
        }
        assert_eq!((report.counted_when_applied[0], report.counted_when_applied[1], report.writes_applied.writes_refused), (own_applied, other_applied, refused_count), "on {thread_count} threads");
    }
}
