//! The entities read: the going over those waking -- a rule of an
//! entity is written for one entity.

use chunk_storage::LayerType;
use entity_manager::EntityRef;
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
