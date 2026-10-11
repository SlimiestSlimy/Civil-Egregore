//! What an entity is: a header -- its type, its cell, when it next
//! wakes -- and its attributes, a run of blocks that may grow or
//! shrink at run time. It has no ID: no two stand on one cell, so the
//! cell it stands on names it (`docs/entity_manager.md`,
//! "Instructions", An entity's name in a tick).

use crate::attributes::{attribute_blocks, Attribute, AttributeBlock, AttributeType, Layout};
use coordinates::CellIndex;
pub use type_registry::EntityType;

/// The tick an entity that never wakes wakes at.
pub const NEVER: u64 = u64::MAX;

/// An entity's fixed part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Its type.
    pub kind: EntityType,
    /// The cell it stands on.
    pub at: CellIndex,
    /// The tick it next wakes at, or [`NEVER`].
    pub wake: u64,
}

/// An entity as stored: its header and its attributes, sorted by type.
#[derive(Clone, Copy, Debug)]
pub struct EntityRef<'a> {
    /// Its fixed part.
    pub header: Header,
    /// Its attributes' blocks, sorted by type.
    pub attributes: &'a [AttributeBlock],
}

impl<'a> EntityRef<'a> {
    /// What its attribute `attribute` holds, if it has it.
    #[inline]
    pub fn attribute<L: Layout>(&self, attribute: Attribute<L>) -> Option<L> {
        crate::attributes::attribute(self.attributes, attribute)
    }

    /// The blocks of its attribute of type `kind`, if it has one: how
    /// one whose size varies is read.
    pub fn attribute_blocks(&self, kind: AttributeType) -> Option<&'a [AttributeBlock]> {
        attribute_blocks(self.attributes, kind)
    }
}
