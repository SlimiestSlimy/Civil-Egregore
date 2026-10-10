//! The entities: the going over those waking -- a rule of an entity
//! is written for one entity -- what one comes to, each by the
//! instruction that carries least, and those put on the world between
//! two ticks, on no superchunk's turn.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeBlock, Entities, EntityEdit, EntityId, EntityRef, EntityType, Header};
use simulation::Turn;

/// Runs `each` on every entity of the turn's superchunk waking this
/// tick, in Morton order, with `state`, the cells of `layers` about
/// each asked of memory ahead. One that is to wake again must be put
/// back with a later wake tick (`docs/instructions.md`, "The going over").
#[inline]
pub fn each_woken<'a, const N: usize, S>(turn: &mut Turn<'a>, layers: [LayerType; N], state: &mut S, mut each: impl FnMut(&mut Turn<'a>, EntityRef<'a>, &mut S)) {
    for entity in turn.woken_reading(layers) {
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

/// Queues `entity` sleeping where it stands until `wake`: its
/// attributes as they are, none carried.
#[inline]
pub fn sleep(turn: &mut Turn, entity: &Header, wake: u64) {
    turn.step(entity, entity.at, wake);
}

/// Queues what `edit`'s entity came to: on `to`, to wake at `wake`,
/// by the instruction that carries least -- moved or put to sleep
/// with the attributes it has if none was changed, else put whole.
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
