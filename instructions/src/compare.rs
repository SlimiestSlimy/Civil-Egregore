//! Compares: what a rule saw -- of a cell or of an entity -- queued
//! as an instruction of its own. If it no longer holds when it is
//! come to, what the rule queued after it is skipped, up to its next
//! compare or the end of the compare: one compare, any number of
//! writes (`docs/instructions.md`, "Compare-and-write").

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
    Compare::attribute(entity.at, attribute.attribute_type(), seen.map(|_| &blocks[..]))
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

/// From here on, what the rule queues -- cells written, what
/// entities come to, counts -- is under `compare`: skipped if it does
/// not hold when come to, up to the next compare given or [`end`].
/// It lands wherever it lands, the compare's superchunk or the
/// next: a border changes nothing of it.
#[inline]
pub fn from_here(turn: &mut Turn, compare: Compare) {
    turn.compare_from_here(compare);
}

/// From here on, what the rule queues is under no compare.
#[inline]
pub fn end(turn: &mut Turn) {
    turn.compare_end();
}
