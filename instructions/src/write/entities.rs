//! The entities written: what one comes to -- made, asleep, changed,
//! removed -- each by the instruction that carries least.

use coordinates::CellIndex;
use entity_manager::{Attribute, EntityEdit, EntityId, EntityType, Header};
use simulation::Turn;

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
