//! The entities: the going over those waking -- a rule of an entity
//! is written for one entity -- what one comes to, each by the
//! instruction that carries least, and those put on the world between
//! two ticks, on no superchunk's turn.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{Attribute, AttributeBlock, Entities, EntityRef, EntityType, Header, Layout};
use simulation::Turn;

/// Runs `each` on every entity of the turn's superchunk waking this
/// tick, in Morton order, with `state`, the cells of `layers` about
/// each asked of memory ahead. One that is to wake again must be put
/// back with a later wake tick (`docs/actions.md`, "The going over").
#[inline]
pub fn each_woken<'a, const N: usize, S>(turn: &mut Turn<'a>, layers: [LayerType; N], state: &mut S, mut each: impl FnMut(&mut Turn<'a>, EntityRef<'a>, &mut S)) {
    for entity in turn.woken_reading(layers) {
        turn.seeing_to(entity.header.at);
        each(turn, entity, state);
    }
}

/// The entity that stood on `at` -- any cell in reach -- as the tick
/// found it: the cell is its name for the tick, whatever is queued
/// for it found by it.
#[inline]
pub fn on<'a>(turn: &Turn<'a>, at: CellIndex) -> Option<EntityRef<'a>> {
    turn.entity_on(at)
}

/// Queues making an entity of type `kind` on `at`, with `attributes`
/// sorted by type, to wake at `wake`. It is not made if the cell is
/// taken by then.
#[inline]
pub fn spawn(turn: &mut Turn, kind: EntityType, at: CellIndex, wake: u64, attributes: &[AttributeBlock]) {
    turn.put(Header { kind, at, wake }, attributes);
}

/// [`spawn`], the compare of the entity's cell left open: what the
/// rule queues from here to [`made`] happens only if the entity is
/// made, and the entity is made only if every compare the rule is
/// queuing under holds -- whichever superchunks the cell and the rest
/// are in. How a rule has two things happen together or not at all:
/// a lamb born and its mother no longer pregnant.
#[inline]
pub fn make(turn: &mut Turn, kind: EntityType, at: CellIndex, wake: u64, attributes: &[AttributeBlock]) {
    turn.make(Header { kind, at, wake }, attributes);
}

/// Ends what [`make`] began: what the rule queues from here does not
/// hang on the entity being made.
#[inline]
pub fn made(turn: &mut Turn) {
    turn.made();
}

/// Queues `entity` sleeping where it stands until `wake`: its
/// attributes as they are, none carried.
#[inline]
pub fn sleep(turn: &mut Turn, entity: &Header, wake: u64) {
    turn.step(entity, entity.at, wake);
}

/// Queues `entity` stepping to `to` -- its own cell to stay where it
/// stands -- to wake at `wake`: its attributes as they are, none
/// carried. Its cell taken by then, it stays, and wakes at `wake`
/// all the same.
#[inline]
pub fn step(turn: &mut Turn, entity: &Header, to: CellIndex, wake: u64) {
    turn.step(entity, to, wake);
}

/// Queues removing `entity`.
#[inline]
pub fn remove(turn: &mut Turn, entity: &Header) {
    turn.remove(entity);
}

/// Queues `attribute` of `other` -- the rule's own entity or
/// another, any in reach -- becoming `value`, if it is still the `seen`
/// the rule read of it, or it still has none: how one entity acts on
/// another. Applied or refused whole, whatever else is written to
/// `other` in the tick -- by itself, awake, or by a third
/// (`docs/actions.md`, "One entity writing another").
#[inline]
pub fn set_attribute_of<L: Layout>(turn: &mut Turn, other: &Header, attribute: Attribute<L>, seen: Option<L>, value: L) {
    turn.set_attribute(other, attribute, seen, value);
}

/// Queues removing `attribute` of `other`, another entity in reach,
/// if it is still the `seen` the rule read of it.
#[inline]
pub fn unset_attribute_of<L: Layout>(turn: &mut Turn, other: &Header, attribute: Attribute<L>, seen: L) {
    turn.unset_attribute(other, attribute, seen);
}

/// The world's entities between two ticks: where a rule puts the ones a
/// world starts with. Whoever runs the world lends it, and places what
/// was put before the next tick.
pub struct EntitiesBetweenTicks<'a> {
    /// The world's entities.
    entities: &'a mut Entities,
    /// Whether a cell is free to put an entity on: not held in the
    /// world's collision plane. Every cell, if none is given.
    free: Option<&'a dyn Fn(CellIndex) -> bool>,
    /// The cells entities were put on: for whoever runs the world to
    /// set in its collision plane.
    cells_put: Vec<CellIndex>,
}

impl<'a> EntitiesBetweenTicks<'a> {
    /// Those of a world whose entities are `entities`: made by whoever
    /// runs the world, never by a rule.
    pub fn of(entities: &'a mut Entities) -> Self {
        Self { entities, free: None, cells_put: Vec::new() }
    }

    /// The same, putting an entity only on a cell `free` says is
    /// free: one the world's collision plane does not hold.
    pub fn where_free(self, free: &'a dyn Fn(CellIndex) -> bool) -> Self {
        Self { free: Some(free), ..self }
    }

    /// The cells entities were put on, in the order put.
    pub fn cells_put(&self) -> &[CellIndex] {
        &self.cells_put
    }

    /// The tick the world is at.
    pub fn now(&self) -> u64 {
        self.entities.now()
    }

    /// Queues a put of an entity whole, header and attributes: in the
    /// world once whoever runs it places what was put. Not queued, and
    /// false, if its cell is not free.
    pub fn put(&mut self, header: Header, attributes: &[AttributeBlock]) -> bool {
        if self.free.is_some_and(|free| !free(header.at)) {
            return false;
        }
        self.entities.queue_put(header, attributes);
        self.cells_put.push(header.at);
        true
    }
}
