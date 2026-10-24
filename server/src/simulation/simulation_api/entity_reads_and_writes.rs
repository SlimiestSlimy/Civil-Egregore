//! The entities: the going over those waking -- a rule of an entity
//! is written for one entity -- what one comes to, each by the
//! instruction that carries least.

mod attribute_edits;

pub use attribute_edits::{set_attribute_blocks, set_attribute_member, set_attribute_of, unset_attribute_of, update_entity};

use crate::chunk_storage::LayerType;
use utilities::coordinates::CellIndex;
use crate::entity_manager::{EntityRef, EntityType};
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attributes::AttributeBlock;
use crate::simulation::simulation_api::compare_and_write::{self, SkipTagSet};
use crate::simulation::Write;
use crate::simulation::SimulationTickState;

/// Runs `entity_rule` on every entity of the turn's superchunk waking this
/// tick, in Morton order, with `rule_state`, the cells of `layers` about
/// each asked of memory ahead. One that is to wake again must be put
/// back with a later wake tick (`docs/actions.md`, "The going over").
#[inline]
pub fn each_woken<'a, const N: usize, S>(tick_state: &mut SimulationTickState<'a>, layers: [LayerType; N], rule_state: &mut S, mut entity_rule: impl FnMut(&mut SimulationTickState<'a>, EntityRef<'a>, &mut S)) {
    for entity in tick_state.woken_reading(layers) {
        tick_state.seeing_to(entity.base_data.stood_cell);
        entity_rule(tick_state, entity, rule_state);
    }
}

/// The entity that stood on `entity_cell` -- any cell in reach -- as the tick
/// found it: the cell is its name for the tick, whatever is queued
/// for it found by it.
#[inline]
pub fn entity_on<'a>(tick_state: &SimulationTickState<'a>, entity_cell: CellIndex) -> Option<EntityRef<'a>> {
    tick_state.entity_on(entity_cell)
}

/// Queues making an entity of type `entity_type` on `entity_cell`, with `attributes`
/// sorted by type, to wake at `wake`. It is not made if the cell is
/// taken by then.
#[inline]
pub fn make_entity(tick_state: &mut SimulationTickState, entity_type: EntityType, entity_cell: CellIndex, wake: u64, attributes: &[AttributeBlock]) {
    drop(begin_making_entity(tick_state, entity_type, entity_cell, wake, attributes));
}

/// [`make_entity`], the compare of the entity's cell left open: what the
/// rule queues under the compares returned happens only if the entity
/// is made, and the entity is made only if every compare the rule is
/// queuing under holds -- whichever superchunks the cell and the rest
/// are in. How a rule has two things happen together or not at all:
/// a lamb born and its mother no longer pregnant.
#[inline]
pub fn begin_making_entity<'scope, 'world>(tick_state: &'scope mut SimulationTickState<'world>, entity_type: EntityType, entity_cell: CellIndex, wake: u64, attributes: &[AttributeBlock]) -> SkipTagSet<'scope, 'world> {
    let mut cell_free_still = compare_and_write::open_compare(tick_state, compare_and_write::cell_free(entity_cell));
    cell_free_still.make_entity(EntityBaseData { entity_type, stood_cell: entity_cell, wake }, attributes);
    cell_free_still
}

/// Queues `entity` sleeping where it stands until `wake`: its
/// attributes as they are, none carried.
#[inline]
pub fn sleep(tick_state: &mut SimulationTickState, entity: &EntityBaseData, wake: u64) {
    step(tick_state, entity, entity.stood_cell, wake);
}

/// Queues `entity` stepping to `to_cell` -- its own cell to stay where it
/// stands -- to wake at `wake`: its attributes as they are, none
/// carried. Its cell taken by then, it stays, and wakes at `wake`
/// all the same. The same whether `to_cell` is in its superchunk or over
/// a border: the simulation's to see to
/// (`../../simulation/docs/simulation.md`, "An entity over a border").
#[inline]
pub fn step(tick_state: &mut SimulationTickState, entity: &EntityBaseData, to_cell: CellIndex, wake: u64) {
    let move_and_wake = [Write::MoveEntity { from_cell: entity.stood_cell, to_cell }, Write::Wake { named_cell: entity.stood_cell, wake }];
    // Sleeping where it stands, it is not moved.
    compare_and_write::with_writes(tick_state, compare_and_write::entity(entity), &move_and_wake[usize::from(to_cell == entity.stood_cell)..]);
}

/// Queues removing `entity`.
#[inline]
pub fn remove(tick_state: &mut SimulationTickState, entity: &EntityBaseData) {
    compare_and_write::with_writes(tick_state, compare_and_write::entity(entity), &[Write::RemoveEntity(entity.stood_cell)]);
}
