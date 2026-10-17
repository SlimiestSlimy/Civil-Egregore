//! Compares: what a rule saw -- of a cell or of an entity -- queued
//! as an instruction of its own. If it no longer holds when it is
//! come to, what the rule queued after it is skipped, up to its next
//! compare or the end of the compare: one compare, any number of
//! writes (`docs/compares_and_writes.md`, "Compares").

use chunk_storage::{LayerType, WidePlane, Width};
use coordinates::CellIndex;
use entity_manager::{push_attribute, Attribute, AttributeBlock, AttributeType, Header, Layout, Member};
pub use crate::Compare;
use crate::{SuperchunkTickState, Write};

/// `layer_type` still holds at `cell`, as the rule saw.
#[inline]
pub fn layer_holds(layer_type: LayerType, cell: CellIndex) -> Compare {
    Compare::Cell { layer_type, target_cell: cell, seen_value: 1 }
}

/// `layer_type` still does not hold at `cell`, as the rule saw.
#[inline]
pub fn layer_lacks(layer_type: LayerType, cell: CellIndex) -> Compare {
    Compare::Cell { layer_type, target_cell: cell, seen_value: 0 }
}

/// `wide_plane` still holds the number `seen_value` at `cell`.
#[inline]
pub fn value<W: Width>(wide_plane: WidePlane<W>, cell: CellIndex, seen_value: u32) -> Compare {
    Compare::Cell { layer_type: wide_plane.layer_type(), target_cell: cell, seen_value }
}

/// `entity` still stands where it stood, its `attribute` what the
/// rule `seen_attribute` -- or it still has none, if it saw none.
#[inline]
pub fn attribute<L: Layout>(entity: &Header, attribute: Attribute<L>, seen_attribute: Option<L>) -> Compare {
    let mut seen_blocks = Vec::new();
    seen_attribute.into_iter().for_each(|seen_attribute_data| push_attribute(&mut seen_blocks, attribute, seen_attribute_data));
    Compare::attribute(entity.stood_cell, attribute.attribute_type(), seen_attribute.map(|_| &seen_blocks[..]))
}

/// The entity `entity` -- its type, on the cell the tick found it on
/// -- is there still, wherever it stands now.
#[inline]
pub fn entity(entity: &Header) -> Compare {
    Compare::Entity { named_cell: entity.stood_cell, entity_type: entity.entity_type }
}

/// An entity may be made on `cell`: nobody stands on it, it is
/// nobody's name this tick, the collision plane does not hold it.
#[inline]
pub fn cell_free(cell: CellIndex) -> Compare {
    Compare::CellFree { target_cell: cell }
}

/// `member` of the attribute of type `attribute_type` of the entity the tick
/// found on `entity_cell` still holds `seen_member_value`.
#[inline]
pub fn attribute_member(entity_cell: CellIndex, attribute_type: AttributeType, member: Member, seen_member_value: u64) -> Compare {
    Compare::AttributeMember { named_cell: entity_cell, attribute_type, member, seen_value: seen_member_value }
}

/// The attribute of the entity the tick found on `entity_cell` is still the
/// blocks `seen_blocks` -- or it still has none of type `attribute_type`, if none was
/// seen.
#[inline]
pub fn attribute_blocks(entity_cell: CellIndex, attribute_type: AttributeType, seen_blocks: Option<&[AttributeBlock]>) -> Compare {
    Compare::attribute(entity_cell, attribute_type, seen_blocks)
}

/// Queues `compare_to_hold`, the `writes` under it and its end in one call:
/// what opening it, a write each and closing it come to, for a compare
/// whose writes all land in its own superchunk -- most of what is
/// queued.
#[inline]
pub fn with_writes(tick_state: &mut SuperchunkTickState, compare_to_hold: Compare, writes: &[Write]) {
    tick_state.compare_with_writes(compare_to_hold, writes);
}
