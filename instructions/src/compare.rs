//! Compares: what a write is held against as it is applied -- a cell
//! or an entity as the rule saw it, the one written or another -- and
//! what is done only if one holds: a cell written, what an entity
//! comes to, a count (`docs/instructions.md`, "Compare-and-write").

use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use entity_manager::{push_attribute, Attribute, Header, Layout};
pub use simulation::Compare;
use simulation::Turn;

/// `layer_type` still holds at `cell`, as the rule saw.
#[inline]
pub fn holds(layer_type: LayerType, cell: CellIndex) -> Compare {
    Compare::Cell { layer_type, at: cell, seen: 1 }
}

/// `layer_type` still does not hold at `cell`, as the rule saw.
#[inline]
pub fn lacks(layer_type: LayerType, cell: CellIndex) -> Compare {
    Compare::Cell { layer_type, at: cell, seen: 0 }
}

/// `plane` still holds the number `seen` at `cell`.
#[inline]
pub fn value<W: Width>(plane: Wide<W>, cell: CellIndex, seen: u32) -> Compare {
    Compare::Cell { layer_type: plane.layer_type(), at: cell, seen: seen as u16 }
}

/// `entity` still stands where it stood, its `attribute` what the
/// rule `seen` -- or it still has none, if it saw none.
#[inline]
pub fn attribute<L: Layout>(entity: &Header, attribute: Attribute<L>, seen: Option<L>) -> Compare {
    let mut blocks = Vec::new();
    seen.into_iter().for_each(|seen| push_attribute(&mut blocks, attribute, seen));
    Compare::attribute(entity.id, entity.at, attribute.attribute_type(), seen.map(|_| &blocks[..]))
}

/// Queues the number of `layer_type` at `cell` becoming `value` if
/// it is still the `seen` the rule read there and `compare` holds too
/// when the write is come to: one added to the rule's count `counted`
/// then, if one is given. The compare is of the cell's superchunk.
#[inline]
pub fn write(turn: &mut Turn, compare: Compare, layer_type: LayerType, cell: CellIndex, seen: u32, value: u32, counted: Option<usize>) {
    turn.queue_if(compare, layer_type, cell, seen, value, counted.map(|place| place as u32));
}

/// Adds one to the rule's count `counted` if `compare` holds when it
/// is come to.
#[inline]
pub fn count(turn: &mut Turn, compare: Compare, counted: usize) {
    turn.count_if(compare, counted as u32);
}

/// From here on, what the rule queues of entities
/// (`entities::commit`, `remove`, ...) is under `compare`: applied
/// only if it holds when come to. They land in the compare's
/// superchunk.
#[inline]
pub fn entities_from_here(turn: &mut Turn, compare: Compare) {
    turn.instructions_if(compare);
}

/// From here on, what the rule queues of entities is under no compare.
#[inline]
pub fn entities_as_ever(turn: &mut Turn) {
    turn.instructions_as_ever();
}
