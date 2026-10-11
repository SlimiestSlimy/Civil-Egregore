//! Compares: what a rule saw -- of a cell or of an entity -- queued
//! as an instruction of its own. If it no longer holds when it is
//! come to, what the rule queued after it is skipped, up to its next
//! compare or the end of the compare: one compare, any number of
//! writes (`docs/compares_and_writes.md`, "Compares").

use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use entity_manager::{push_attribute, Attribute, AttributeBlock, AttributeType, Header, Layout, Member};
pub use simulation::Compare;
use simulation::{Turn, Write};

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

/// The entity `entity` -- its type, on the cell the tick found it on
/// -- is there still, wherever it stands now.
#[inline]
pub fn entity(entity: &Header) -> Compare {
    Compare::Entity { at: entity.at, kind: entity.kind }
}

/// An entity may be made on `cell`: nobody stands on it, it is
/// nobody's name this tick, the collision plane does not hold it.
#[inline]
pub fn cell_free(cell: CellIndex) -> Compare {
    Compare::CellFree { at: cell }
}

/// `member` of the attribute of type `kind` of the entity the tick
/// found on `at` still holds `seen`.
#[inline]
pub fn attribute_member(at: CellIndex, kind: AttributeType, member: Member, seen: u64) -> Compare {
    Compare::AttributeMember { at, kind, member, seen }
}

/// The attribute of the entity the tick found on `at` is still the
/// blocks `seen` -- or it still has none of type `kind`, if none was
/// seen.
#[inline]
pub fn attribute_blocks(at: CellIndex, kind: AttributeType, seen: Option<&[AttributeBlock]>) -> Compare {
    Compare::attribute(at, kind, seen)
}

/// Queues `compare`, open from here: what is queued until [`close`]
/// is under it, and under every compare open already.
#[inline]
pub fn open(turn: &mut Turn, compare: Compare) {
    turn.compare_open(compare);
}

/// Queues the end of the compare opened last by [`open`].
#[inline]
pub fn close(turn: &mut Turn) {
    turn.compare_close();
}

/// Queues `compare`, the `writes` under it and its end in one call:
/// what [`open`], a write each and [`close`] come to, for a compare
/// whose writes all land in its own superchunk -- most of what is
/// queued.
#[inline]
pub fn with_writes(turn: &mut Turn, compare: Compare, writes: &[Write]) {
    turn.compare_with_writes(compare, writes);
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
