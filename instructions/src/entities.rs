//! The entities: the going over those waking -- a rule of an entity is
//! written for one entity -- and what one comes to: made, asleep,
//! changed, removed, each by the instruction that carries least.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{Attribute, EntityEdit, EntityId, EntityRef, EntityType, Header};
use simulation::Turn;

/// Runs `each` on every entity of the turn's superchunk waking this
/// tick, in Morton order by cell, with `state`: what the rule counts,
/// and whatever it keeps from one entity to the next. The cells of
/// `layers` about each are asked of memory a few wakes ahead: those
/// the rule reads. Each that is to wake again must be put back with a
/// later wake tick.
#[inline]
pub fn each_woken<'a, const N: usize, S>(turn: &mut Turn<'a>, layers: [LayerType; N], state: &mut S, mut each: impl FnMut(&mut Turn<'a>, EntityRef<'a>, &mut S)) {
    for entity in turn.woken_reading(layers) {
        each(turn, entity, state);
    }
}

/// Queues making an entity of type `kind` on `at`, with `attributes`
/// sorted by type, to wake at `wake`: its ID, drawn here. It is not
/// made if an entity stands on the cell by then.
#[inline]
pub fn spawn(turn: &mut Turn, kind: EntityType, at: CellIndex, wake: u64, attributes: &[Attribute]) -> EntityId {
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
