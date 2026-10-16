//! The entities: the going over those waking -- a rule of an entity
//! is written for one entity -- what one comes to, each by the
//! instruction that carries least, and those put on the world between
//! two ticks, on no superchunk's turn.

mod attribute_edits;

pub use attribute_edits::{set_attribute_blocks, set_attribute_member, set_attribute_of, unset_attribute_of, update_entity};

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{each_attribute, AttributeBlock, WorldEntities, EntityRef, EntityType, Header};
use compares_and_writes::{compare, entity, write, Write};
use simulation::Turn;

/// Runs `entity_rule` on every entity of the turn's superchunk waking this
/// tick, in Morton order, with `rule_state`, the cells of `layers` about
/// each asked of memory ahead. One that is to wake again must be put
/// back with a later wake tick (`docs/actions.md`, "The going over").
#[inline]
pub fn each_woken<'a, const N: usize, S>(turn: &mut Turn<'a>, layers: [LayerType; N], rule_state: &mut S, mut entity_rule: impl FnMut(&mut Turn<'a>, EntityRef<'a>, &mut S)) {
    for entity in turn.woken_reading(layers) {
        turn.seeing_to(entity.header.stood_cell);
        entity_rule(turn, entity, rule_state);
    }
}

/// The entity that stood on `entity_cell` -- any cell in reach -- as the tick
/// found it: the cell is its name for the tick, whatever is queued
/// for it found by it.
#[inline]
pub fn entity_on<'a>(turn: &Turn<'a>, entity_cell: CellIndex) -> Option<EntityRef<'a>> {
    turn.entity_on(entity_cell)
}

/// Queues making an entity of type `entity_type` on `entity_cell`, with `attributes`
/// sorted by type, to wake at `wake`. It is not made if the cell is
/// taken by then.
#[inline]
pub fn make_entity(turn: &mut Turn, entity_type: EntityType, entity_cell: CellIndex, wake: u64, attributes: &[AttributeBlock]) {
    begin_making_entity(turn, entity_type, entity_cell, wake, attributes);
    end_making_entity(turn);
}

/// [`make_entity`], the compare of the entity's cell left open: what the
/// rule queues from here to [`end_making_entity`] happens only if the entity is
/// made, and the entity is made only if every compare the rule is
/// queuing under holds -- whichever superchunks the cell and the rest
/// are in. How a rule has two things happen together or not at all:
/// a lamb born and its mother no longer pregnant.
#[inline]
pub fn begin_making_entity(turn: &mut Turn, entity_type: EntityType, entity_cell: CellIndex, wake: u64, attributes: &[AttributeBlock]) {
    compare::open_compare(turn, compare::cell_free(entity_cell));
    entity::queue_entity_made(turn, Header { entity_type, stood_cell: entity_cell, wake });
    for attribute in each_attribute(attributes) {
        write::attribute_whole(turn, entity_cell, attribute);
    }
}

/// Ends what [`begin_making_entity`] began: what the rule queues from here does not
/// hang on the entity being made.
#[inline]
pub fn end_making_entity(turn: &mut Turn) {
    compare::close_compare(turn);
}

/// Queues `entity` sleeping where it stands until `wake`: its
/// attributes as they are, none carried.
#[inline]
pub fn sleep(turn: &mut Turn, entity: &Header, wake: u64) {
    step(turn, entity, entity.stood_cell, wake);
}

/// Queues `entity` stepping to `to_cell` -- its own cell to stay where it
/// stands -- to wake at `wake`: its attributes as they are, none
/// carried. Its cell taken by then, it stays, and wakes at `wake`
/// all the same. The same whether `to_cell` is in its superchunk or over
/// a border: the simulation's to see to
/// (`../../simulation/docs/simulation.md`, "An entity over a border").
#[inline]
pub fn step(turn: &mut Turn, entity: &Header, to_cell: CellIndex, wake: u64) {
    let move_and_wake = [Write::MoveEntity { from_cell: entity.stood_cell, to_cell }, Write::Wake { named_cell: entity.stood_cell, wake }];
    // Sleeping where it stands, it is not moved.
    compare::with_writes(turn, compare::entity(entity), &move_and_wake[usize::from(to_cell == entity.stood_cell)..]);
}

/// Queues removing `entity`.
#[inline]
pub fn remove(turn: &mut Turn, entity: &Header) {
    compare::with_writes(turn, compare::entity(entity), &[Write::RemoveEntity(entity.stood_cell)]);
}

/// The world's entities between two ticks: where a rule puts the ones a
/// world starts with. Whoever runs the world lends it, and places what
/// was put before the next tick.
pub struct EntitiesBetweenTicks<'a> {
    /// The world's entities.
    world_entities: &'a mut WorldEntities,
    /// Whether a cell is free to put an entity on: not held in the
    /// world's collision plane. Every cell, if none is given.
    is_cell_free: Option<&'a dyn Fn(CellIndex) -> bool>,
    /// The cells entities were put on: for whoever runs the world to
    /// set in its collision plane.
    cells_put: Vec<CellIndex>,
}

impl<'a> EntitiesBetweenTicks<'a> {
    /// Those of a world whose entities are `world_entities`: made by whoever
    /// runs the world, never by a rule.
    pub fn from_entities(world_entities: &'a mut WorldEntities) -> Self {
        Self { world_entities, is_cell_free: None, cells_put: Vec::new() }
    }

    /// The same, putting an entity only on a cell `is_cell_free` says is
    /// free: one the world's collision plane does not hold.
    pub fn where_free(self, is_cell_free: &'a dyn Fn(CellIndex) -> bool) -> Self {
        Self { is_cell_free: Some(is_cell_free), ..self }
    }

    /// The cells entities were put on, in the order put.
    pub fn cells_put(&self) -> &[CellIndex] {
        &self.cells_put
    }

    /// The tick the world is at.
    pub fn current_tick(&self) -> u64 {
        self.world_entities.current_tick()
    }

    /// Queues a put of an entity whole, header and attributes: in the
    /// world once whoever runs it places what was put. Not queued, and
    /// false, if its cell is not free.
    pub fn put(&mut self, header: Header, attributes: &[AttributeBlock]) -> bool {
        if self.is_cell_free.is_some_and(|is_cell_free| !is_cell_free(header.stood_cell)) {
            return false;
        }
        self.world_entities.queue_put(header, attributes);
        self.cells_put.push(header.stood_cell);
        true
    }
}
