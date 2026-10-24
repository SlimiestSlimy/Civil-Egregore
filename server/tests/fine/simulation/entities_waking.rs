//! Entities ticked: woken at their tick and no other, in Morton order;
//! attributes added and removed at run time; changes queued outside a
//! tick; and the same on any number of threads.
//!
//! `cargo test`

use crate::arenas::*;
use crate::test_types::TestTypes;
use crate::walkers::*;
use utilities::coordinates::{SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use crate::entity_manager::WHEEL_TICKS;
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attributes::AttributeBlock;
use crate::simulation::{Simulation, SimulationTickState};
use std::sync::Mutex;

/// Attributes added and removed every tick, by hundreds of entities in
/// one chunk -- so the old runs pile up as garbage and are swept -- come
/// out right.
#[test]
fn attributes_come_and_go_at_run_time() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let (mut bitmap_arena, mut entity_store) = world(types, 1);
    for walker_cell in cells_drawn(seed, 300, 1024) {
        entity_store.queue_add(walker_base_data(types, walker_cell, 0), &[]);
    }
    assert_eq!(entity_store.queued_count(), 300);
    assert_eq!(entity_store.apply().added, 300);
    assert_eq!((entity_store.len(), entity_store.queued_count()), (300, 0));
    let mut world_simulation = Simulation::new(1);
    for _ in 1..=101u64 {
        let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| count_and_flip(types, tick_state));
        assert_eq!((report.rules, report.instructions_applied.wakes), (300, 300));
    }
    assert_eq!(entity_store.len(), 300);
    for entity in entity_store.iter() {
        assert_eq!(entity.attribute(types.woken_count), Some(101));
        assert_eq!(entity.attribute(types.odd_wake), Some(1), "flipped 101 times");
        assert_eq!(entity.attributes.len(), 2);
    }
}

/// An entity wakes at its tick and no other: one in reach of the wheel,
/// one past it, and none once removed.
#[test]
fn entities_wake_at_their_tick() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let (mut bitmap_arena, mut entity_store) = world(types, 1);
    let far_wake = 2 * WHEEL_TICKS + 300;
    let (first_cell, far_cell, removed_cell) = (cell(3, 3), cell(900, 900), cell(10, 10));
    entity_store.queue_add(walker_base_data(types, first_cell, 5), &[]);
    entity_store.queue_add(walker_base_data(types, far_cell, far_wake), &[]);
    entity_store.queue_add(walker_base_data(types, removed_cell, 7), &[]);
    entity_store.apply();
    let woken_log = Mutex::new(Vec::new());
    let mut world_simulation = Simulation::new(1);
    for _ in 0..=far_wake + 10 {
        world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state| {
            for entity in tick_state.woken_entities() {
                woken_log.lock().unwrap().push((entity.base_data.stood_cell, tick_state.current_tick()));
                if entity.base_data.stood_cell == removed_cell {
                    crate::simulation::simulation_api::entity_reads_and_writes::remove(tick_state, &entity.base_data);
                } else if entity.base_data.stood_cell == first_cell && tick_state.current_tick() == 5 {
                    crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &entity.base_data, first_cell, 40);
                }
            }
            0
        });
    }
    assert_eq!(woken_log.into_inner().unwrap(), [(first_cell, 5), (removed_cell, 7), (first_cell, 40), (far_cell, far_wake)]);
    assert_eq!(entity_store.len(), 2, "the third removed");
}

/// Walkers stepping right a cell a tick cross into the next superchunk,
/// as whole copies, attributes and all -- the one left behind gone by
/// the tick's end -- and at the edge of the hot superchunks stay, lost
/// to nowhere.
#[test]
fn entities_cross_borders_and_stay_at_the_edge_of_the_hot_world() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let (mut bitmap_arena, mut entity_store) = world(types, 2);
    let start_column = SUPERCHUNK_SIDE_CELLS - 3;
    entity_store.queue_add(walker_base_data(types, cell(start_column, 100), 0), &[AttributeBlock::holding_value(types.woken_count, 0)]);
    entity_store.apply();
    let mut world_simulation = Simulation::new(2);
    let step = |tick_state: &mut SimulationTickState| {
        for entity in tick_state.woken_entities() {
            let stepped_base_data = EntityBaseData { stood_cell: entity.base_data.stood_cell.at_cartesian_offset(1, 0).unwrap(), wake: tick_state.current_tick() + 1, ..entity.base_data };
            crate::simulation::simulation_api::entity_reads_and_writes::update_entity(tick_state, &entity.base_data, stepped_base_data, &[AttributeBlock::holding_value(types.woken_count, entity.attribute(types.woken_count).unwrap() + 1)]);
        }
        0
    };
    let mut most_entities = 0;
    for _ in 0..10 {
        world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, step);
        most_entities = most_entities.max(entity_store.len());
    }
    assert_eq!(most_entities, 1, "never here and there at a tick's end");
    let right_superchunk = SuperchunkIndex::from_cartesian(11, 10);
    let entity = entity_store.superchunk(right_superchunk).and_then(|superchunk| superchunk.iter().next()).expect("in the right neighbour");
    assert_eq!((entity.base_data.stood_cell, entity.attribute(types.woken_count)), (cell(start_column + 10, 100), Some(10)));
    for _ in 10..1100 {
        world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, step);
    }
    let all_cells: Vec<_> = entity_store.iter().map(|entity| entity.base_data.stood_cell).collect();
    assert_eq!(all_cells, [cell(2 * SUPERCHUNK_SIDE_CELLS - 1, 100)], "at the edge of the hot superchunks, not lost past it");
}

/// Walkers wandering at random, giving birth and dying over 3x3
/// superchunks, across their borders: the same on one thread and four.
#[test]
fn any_number_of_threads_ticks_entities_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let wander_rule = |tick_state: &mut SimulationTickState| {
        let mut change_count = 0;
        for entity in tick_state.woken_entities() {
            let base_data = entity.base_data;
            let (dx, dy) = (tick_state.random_stream().draw_below(3) as i32 - 1, tick_state.random_stream().draw_below(3) as i32 - 1);
            let wander_cell = base_data.stood_cell.at_cartesian_offset(dx * 7, dy * 7).unwrap();
            let wake = tick_state.current_tick() + 1 + tick_state.random_stream().draw_below(5);
            match tick_state.random_stream().draw_below(40) {
                0 => crate::simulation::simulation_api::entity_reads_and_writes::remove(tick_state, &base_data),
                1 => {
                    crate::simulation::simulation_api::entity_reads_and_writes::make_entity(tick_state, base_data.entity_type, wander_cell, wake, &[]);
                    crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &base_data, base_data.stood_cell, tick_state.current_tick() + 3);
                }
                _ => crate::simulation::simulation_api::entity_reads_and_writes::update_entity(tick_state, &base_data, EntityBaseData { stood_cell: wander_cell, wake, ..base_data }, entity.attributes),
            }
            change_count += 1;
        }
        change_count
    };
    let tick_on_threads = |thread_count| {
        let (mut bitmap_arena, mut entity_store) = world(types, 3);
        for (walker_number, walker_cell) in (0..2000u64).zip(cells_drawn(seed, 2000, 3072)) {
            entity_store.queue_add(walker_base_data(types, walker_cell, walker_number % 4), &[]);
        }
        entity_store.apply();
        let mut world_simulation = Simulation::new(thread_count);
        let total_changes: usize = (0..300).map(|_| world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, wander_rule).rules).sum();
        let all_entities: Vec<(EntityBaseData, Vec<_>)> = entity_store.iter().map(|entity| (entity.base_data, entity.attributes.to_vec())).collect();
        (total_changes, all_entities)
    };
    let (one_thread_result, four_thread_result) = (tick_on_threads(1), tick_on_threads(4));
    assert!(one_thread_result.0 > 10_000 && !one_thread_result.1.is_empty());
    assert_eq!(one_thread_result, four_thread_result);
}

/// Changes queued outside a tick land where their cells are, and one
/// past the hot superchunks is lost; a removal takes its entity out.
#[test]
fn changes_outside_a_tick_are_queued_then_applied() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (_, mut entity_store) = world(types, 1);
    let (here_base_data, away_base_data) = (walker_base_data(types, cell(5, 5), 0), walker_base_data(types, cell(2 * SUPERCHUNK_SIDE_CELLS, 5), 0));
    entity_store.queue_add(here_base_data, &[]);
    entity_store.queue_add(away_base_data, &[]);
    assert_eq!(entity_store.len(), 0, "nothing changes until applied");
    let applied_counts = entity_store.apply();
    assert_eq!((applied_counts.added, applied_counts.lost, entity_store.len()), (1, 1, 1));
    assert_eq!(entity_store.get(here_base_data.stood_cell).map(|entity| entity.base_data), Some(here_base_data));
    entity_store.queue_remove(here_base_data.stood_cell);
    assert_eq!((entity_store.apply().entities_removed, entity_store.len()), (1, 0));
}
