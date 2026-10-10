//! The entities: the going over those waking -- a rule of an entity
//! is written for one entity -- what one comes to, each by the
//! instruction that carries least, and those put on the world between
//! two ticks, on no superchunk's turn.

use crate::around;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{Attribute, AttributeBlock, Entities, EntityEdit, EntityId, EntityRef, EntityType, Header, Layout};
use simulation::Turn;

/// Runs `each` on every entity of the turn's superchunk waking this
/// tick, in Morton order, with `state`, the cells of `layers` about
/// each asked of memory ahead. One that is to wake again must be put
/// back with a later wake tick (`docs/instructions.md`, "The going over").
#[inline]
pub fn each_woken<'a, const N: usize, S>(turn: &mut Turn<'a>, layers: [LayerType; N], state: &mut S, mut each: impl FnMut(&mut Turn<'a>, EntityRef<'a>, &mut S)) {
    for entity in turn.woken_reading(layers) {
        turn.seeing_to(entity.header.at);
        each(turn, entity, state);
    }
}

/// Whether the entity whose ID is `id` stood on `at` -- any cell in
/// reach -- as the tick found it.
#[inline]
pub fn stands(turn: &Turn, id: EntityId, at: CellIndex) -> bool {
    turn.entity(id, at).is_some()
}

/// Queues making an entity of type `kind` on `at`, with `attributes`
/// sorted by type, to wake at `wake`: its ID, drawn here. It is not
/// made if an entity stands on the cell by then.
#[inline]
pub fn spawn(turn: &mut Turn, kind: EntityType, at: CellIndex, wake: u64, attributes: &[AttributeBlock]) -> EntityId {
    let id = turn.new_id();
    turn.put(Header { id, kind, at, wake }, attributes);
    id
}

/// The bit, of the nine about `centre`, of the cell of `among` the
/// entity whose ID is `id` stood on as the tick found it, if it stood
/// on one.
#[inline]
pub fn stands_beside(turn: &Turn, id: EntityId, centre: CellIndex, among: u16) -> Option<u32> {
    (0..9).filter(|&bit| among >> bit & 1 == 1).find(|&bit| around::cell(centre, bit).is_some_and(|cell| stands(turn, id, cell)))
}

/// Queues `entity` sleeping where it stands until `wake`: its
/// attributes as they are, none carried.
#[inline]
pub fn sleep(turn: &mut Turn, entity: &Header, wake: u64) {
    turn.step(entity, entity.at, wake);
}

/// Queues what `edit`'s entity came to: on `to`, to wake at `wake`
/// -- each attribute changed a compare-and-write of its own, held
/// against what the rule saw of it, then the entity moved or put to
/// sleep. It is not put whole over itself: what another writes to it
/// in the same tick stands.
#[inline]
pub fn commit(turn: &mut Turn, edit: EntityEdit, to: CellIndex, wake: u64) {
    let before = *edit.header();
    if edit.edited() {
        turn.update(&before, Header { at: to, wake, ..before }, edit.attributes());
    } else {
        turn.step(&before, to, wake);
    }
}

/// Queues removing `entity`.
#[inline]
pub fn remove(turn: &mut Turn, entity: &Header) {
    turn.remove(entity);
}

/// Queues `attribute` of `other` -- another entity than the rule's
/// own, any in reach -- becoming `value`, if it is still the `seen`
/// the rule read of it, or it still has none: how one entity acts on
/// another. Applied or refused whole, whatever else is written to
/// `other` in the tick -- by itself, awake, or by a third
/// (`docs/instructions.md`, "One entity writing another").
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
}

impl<'a> EntitiesBetweenTicks<'a> {
    /// Those of a world whose entities are `entities`: made by whoever
    /// runs the world, never by a rule.
    pub fn of(entities: &'a mut Entities) -> Self {
        Self { entities }
    }

    /// The tick the world is at.
    pub fn now(&self) -> u64 {
        self.entities.now()
    }

    /// Queues a put of an entity whole, header and attributes: in the
    /// world once whoever runs it places what was put.
    pub fn put(&mut self, header: Header, attributes: &[AttributeBlock]) {
        self.entities.queue_put(header, attributes);
    }
}
