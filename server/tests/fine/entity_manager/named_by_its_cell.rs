//! An entity named, all a tick long, by the cell the tick found it on.

use super::{registered_types, walker_base_data, world, cell};
use crate::entity_manager::{Instructions, InstructionsApplied};

/// An entity is named, all a tick long, by the cell the tick found it
/// on: an attribute written to it after it moved lands on it where it
/// now stands, as does its removal; and that cell is no other's until
/// the tick is over -- one stepping to it stays, one new is refused.
/// Each instruction is applied as it is come to, none before another
/// for its kind. An instruction for an entity that is not there is
/// passed over, and counted.
#[test]
fn an_entity_is_named_by_the_cell_the_tick_found_it_on() {
    let (frozen_types, types) = registered_types(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut world_entities = world(&frozen_types);
    let (first_cell, second_cell, third_cell) = (cell(40, 30), cell(41, 30), cell(42, 30));
    world_entities.queue_add(walker_base_data(types, first_cell), &[]);
    world_entities.queue_add(walker_base_data(types, second_cell), &[]);
    world_entities.apply();
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.move_entity(walker_base_data(types, third_cell), first_cell);
    instruction_queue.set_attribute(first_cell, types.one_block, 7);
    instruction_queue.move_entity(walker_base_data(types, first_cell), second_cell);
    instruction_queue.add(walker_base_data(types, first_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.moves, instructions_applied.edits, instructions_applied.turned_back, instructions_applied.adds_refused, instructions_applied.passed_over), (2, 1, 1, 1, 0));
    assert_eq!(world_entities.get(third_cell).and_then(|moved_entity| moved_entity.attribute(types.one_block)), Some(7), "written where it moved to");
    assert!(world_entities.get(second_cell).is_some() && world_entities.len() == 2, "the cell left is no other's this tick");
    // Still the same tick: removed by the name it has in it, and nothing written to it after.
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.remove(first_cell);
    instruction_queue.set_attribute(first_cell, types.one_block, 8);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.entities_removed, instructions_applied.passed_over, world_entities.len()), (1, 1, 1));
    // The tick over, the cells are whoever comes to them.
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.add(walker_base_data(types, first_cell), &[]);
    instruction_queue.add(walker_base_data(types, third_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.added, world_entities.len()), (2, 3));
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());

    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    let nobodys_cell = cell(50, 50);
    instruction_queue.remove(nobodys_cell);
    instruction_queue.unset_attribute(nobodys_cell, types.one_block.attribute_type());
    instruction_queue.move_entity(walker_base_data(types, second_cell), nobodys_cell);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.passed_over, instructions_applied.entities_removed, instructions_applied.edits, instructions_applied.moves, world_entities.len()), (3, 0, 0, 0, 3));
}
