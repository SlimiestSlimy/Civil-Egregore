//! What an entity is: a header -- its ID, its type, its cell, when it
//! next wakes -- and its attributes, a run of blocks that may grow or
//! shrink at run time.

use crate::attributes::{attribute_blocks, remove_attribute, set_attribute, set_attribute_blocks, Attribute, AttributeBlock, AttributeType, Layout};
use coordinates::CellIndex;
pub use type_registry::EntityType;

/// An entity's ID: drawn at random when it is made, from the random
/// numbers of the superchunk making it, so the same on any number of
/// threads. It finds the entity among those in its chunk; two entities
/// near each other sharing one is a chance of about one in 2^64 a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// The tick an entity that never wakes wakes at.
pub const NEVER: u64 = u64::MAX;

/// An entity's fixed part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Its ID.
    pub id: EntityId,
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

/// An entity being changed by its rule: its attributes read, set and
/// removed as if they were its own already, and nothing copied until
/// one is. What it comes to is queued by `instructions::write::entities::commit`,
/// which picks the instruction: an entity whose attributes were left
/// alone is moved, or put back to sleep, and carries none.
pub struct EntityEdit<'a, 'b> {
    /// The entity as the tick found it.
    entity: EntityRef<'a>,
    /// Its attributes as changed, once one is: the rule's room for them,
    /// used again for every entity.
    changed: &'b mut Vec<AttributeBlock>,
    /// Whether one was.
    edited: bool,
}

impl<'a, 'b> EntityEdit<'a, 'b> {
    /// `entity`, to be changed, with `room` for its attributes.
    pub fn of(entity: EntityRef<'a>, room: &'b mut Vec<AttributeBlock>) -> Self {
        Self { entity, changed: room, edited: false }
    }

    /// Its header, as the tick found it.
    pub fn header(&self) -> &Header {
        &self.entity.header
    }

    /// Whether an attribute was set or removed.
    pub fn edited(&self) -> bool {
        self.edited
    }

    /// Its attributes' blocks, as changed so far.
    pub fn attributes(&self) -> &[AttributeBlock] {
        if self.edited { self.changed } else { self.entity.attributes }
    }

    /// What its attribute `attribute` holds, as changed so far.
    #[inline]
    pub fn get<L: Layout>(&self, attribute: Attribute<L>) -> Option<L> {
        crate::attributes::attribute(self.attributes(), attribute)
    }

    /// Sets its attribute `attribute` to `value`.
    #[inline]
    pub fn set<L: Layout>(&mut self, attribute: Attribute<L>, value: L) {
        if self.get(attribute) != Some(value) {
            set_attribute(self.own(), attribute, value);
        }
    }

    /// Sets the attribute `attribute` is the blocks of: how one whose
    /// size varies is written.
    pub fn set_blocks(&mut self, attribute: &[AttributeBlock]) {
        set_attribute_blocks(self.own(), attribute);
    }

    /// Removes its attribute `attribute`: what it held, if it had it.
    #[inline]
    pub fn unset<L: Layout>(&mut self, attribute: Attribute<L>) -> Option<L> {
        let held = self.get(attribute)?;
        remove_attribute(self.own(), attribute.attribute_type());
        Some(held)
    }

    /// Its attributes, copied to be changed if they have not been.
    fn own(&mut self) -> &mut Vec<AttributeBlock> {
        if !self.edited {
            self.changed.clear();
            self.changed.extend_from_slice(self.entity.attributes);
            self.edited = true;
        }
        self.changed
    }
}
