//! What an entity comes to, and the cells beside it: one changed is
//! written only the attributes changed, and the nine cells about it,
//! the free ones among them, are asked as masks.
//!
//! `cargo test -p server --lib fine_tests`

use crate::arenas::{cell, walker_base_data, world};
use crate::test_types::TestTypes;
use utilities::coordinates::CellIndex;
use protocol::server_types::entity::NEVER_WAKES;
use protocol::server_types::attributes::AttributeBlock;
use crate::simulation::simulation_api::cell_and_its_neighbours;
use crate::simulation::simulation_api::{entity_reads_and_writes, SimulationTickState};
use crate::simulation::Simulation;
use std::sync::Mutex;


/// An entity its rule looks over and leaves as it was is moved, or put
/// to sleep, with nothing carried; one with an attribute changed has
/// that attribute written, held against what was seen of it, and is
/// moved the same -- never put whole.
#[test]
fn an_entity_is_written_only_the_attributes_changed() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let (mut bitmap_arena, mut entity_store) = world(types, 1);
    entity_store.queue_add(walker_base_data(types, cell(40, 30), 0), &[AttributeBlock::holding_value(types.name, 7)]);
    entity_store.queue_add(walker_base_data(types, cell(40, 40), 0), &[AttributeBlock::holding_value(types.name, 7)]);
    entity_store.apply();
    let mut test_simulation = Simulation::new(1);
    let report = test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        for entity in tick_state.woken_entities() {
            // The first is left as it was; the second walker's name is changed.
            if entity.base_data.stood_cell == cell(40, 40) {
                entity_reads_and_writes::set_attribute_of(tick_state, &entity.base_data, types.name, entity.attribute(types.name), 8);
            }
            let step_cell = entity.base_data.stood_cell.at_cartesian_offset(1, 1).expect("in the world");
            entity_reads_and_writes::step(tick_state, &entity.base_data, step_cell, NEVER_WAKES);
        }
        0
    });
    assert_eq!((report.instructions_applied.moves, report.instructions_applied.added, report.instructions_applied.edits, report.instructions_compared), (2, 0, 1, (3, 0)));
    assert_eq!(entity_store.get(cell(41, 31)).expect("moved").attributes, [AttributeBlock::holding_value(types.name, 7)]);
    assert_eq!(entity_store.get(cell(41, 41)).expect("moved").attributes, [AttributeBlock::holding_value(types.name, 8)]);
}

/// The cells about an entity: the layer's among them, the ways to
/// each, and one drawn among those chosen.
#[test]
fn the_cells_beside_an_entity_are_asked_as_sets() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let (mut bitmap_arena, mut entity_store) = world(types, 1);
    let walker_cell = cell(40, 30);
    entity_store.queue_add(walker_base_data(types, walker_cell, 0), &[]);
    entity_store.apply();
    let seen_by_rule = Mutex::new(Vec::new());
    let mut test_simulation = Simulation::new(1);
    test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        for entity in tick_state.woken_entities() {
            let woken_cell = entity.base_data.stood_cell;
            let about_tiles = cell_and_its_neighbours::tiles_about_cell(woken_cell);
            let stone_about = cell_and_its_neighbours::read_layer(tick_state, types.stone, &about_tiles);
            // Up and left of it, and to its right: the two chosen among.
            let two_chosen = about_tiles.cell_towards(cell_and_its_neighbours::NORTH_WEST) | about_tiles.cell_towards(cell_and_its_neighbours::EAST);
            let ways_drawn = (0..64).filter_map(|_| cell_and_its_neighbours::pick_way(tick_state.random_stream(), &about_tiles, two_chosen)).fold(0u16, |drawn_so_far, way| drawn_so_far | 1 << way);
            let none_drawn = cell_and_its_neighbours::pick_way(tick_state.random_stream(), &about_tiles, two_chosen.cells_not_in(two_chosen));
            let cells: Vec<CellIndex> = [cell_and_its_neighbours::NORTH_WEST, cell_and_its_neighbours::EAST].iter().filter_map(|&way| cell_and_its_neighbours::cell(woken_cell, way)).collect();
            seen_by_rule.lock().unwrap().push((stone_about.set_cells.count(), stone_about.hot == about_tiles.nine_cells(), ways_drawn, none_drawn, cells, cell_and_its_neighbours::way_of(woken_cell, woken_cell.at_cartesian_offset(-1, 1).unwrap())));
            entity_reads_and_writes::sleep(tick_state, &entity.base_data, NEVER_WAKES);
        }
        0
    });
    assert_eq!(*seen_by_rule.lock().unwrap(), [(0, true, 1 << 0 | 1 << 5, None, vec![cell(39, 29), cell(41, 30)], 6)]);
}
