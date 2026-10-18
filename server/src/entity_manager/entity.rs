//! What an entity is: a header -- its type, its cell, when it next
//! wakes -- and its attribute list, blocks that may grow or
//! shrink at run time. It has no ID: no two stand on one cell, so the
//! cell it stands on names it (`docs/entity_manager.md`,
//! "Instructions", An entity's name in a tick).

use crate::entity_manager::attributes::{attribute_blocks, attribute_bytes, attribute_seen, default_seen, Attribute, AttributeBlock, AttributeType, DefaultAttributes, DefaultKept, Layout, WholeAttribute, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS};
use utilities::coordinates::CellIndex;
pub use protocol::type_registry::EntityType;

/// The tick an entity that never wakes wakes at.
pub const NEVER_WAKES: u64 = u64::MAX;

/// An entity's fixed part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Its type.
    pub entity_type: EntityType,
    /// The cell it stands on.
    pub stood_cell: CellIndex,
    /// The tick it next wakes at, or [`NEVER_WAKES`].
    pub wake: u64,
}

/// An entity as stored: its header and its attributes.
#[derive(Clone, Copy, Debug)]
pub struct EntityRef<'a> {
    /// Its fixed part.
    pub header: Header,
    /// Its attributes' blocks, sorted by type: all but its type's
    /// default attribute, which is read as any other
    /// ([`EntityRef::attribute`]) but kept apart
    /// ([`EntityRef::default_attribute`]).
    pub attributes: &'a [AttributeBlock],
    /// All its blocks as its bucket keeps them: its default
    /// attribute's, if its type has one, then `attributes`.
    pub(crate) kept_blocks: &'a [AttributeBlock],
    /// The world's default attributes: what its type's is, asked only
    /// if it has one.
    pub(crate) default_attributes: &'a DefaultAttributes,
}

impl<'a> EntityRef<'a> {
    /// Its type's default attribute as kept, and what kind it is, if
    /// its type has one: asked of the registry only then.
    #[inline]
    fn default_kept(&self) -> Option<(DefaultKept, &'a [AttributeBlock])> {
        let default_block_count = self.kept_blocks.len() - self.attributes.len();
        if default_block_count == 0 {
            return None;
        }
        Some((self.default_attributes.of(self.header.entity_type)?, &self.kept_blocks[..default_block_count]))
    }

    /// Its type's default attribute as kept, if `attribute_type` is it.
    #[inline]
    fn default_if(&self, attribute_type: AttributeType) -> Option<(DefaultKept, &'a [AttributeBlock])> {
        self.default_kept().filter(|(kept_default, _)| kept_default.attribute_type.type_identity() == attribute_type.type_identity())
    }

    /// What its attribute `attribute` holds, if it has it: its
    /// type's default attribute always.
    #[inline]
    pub fn attribute<L: Layout>(&self, attribute: Attribute<L>) -> Option<L> {
        if let Some((kept_default, default_blocks)) = self.default_if(attribute.attribute_type()) {
            // Its words laid one after another where it is read: nothing taken for them.
            let mut words = [0; ATTRIBUTE_WORDS_MOST];
            for (words, kept_block) in words.as_chunks_mut::<BLOCK_WORDS>().0.iter_mut().zip(default_blocks) {
                *words = kept_block.0;
            }
            return (kept_default.words == L::WORDS).then(|| L::read_data(&words[..L::WORDS]));
        }
        crate::entity_manager::attributes::attribute(self.attributes, attribute)
    }

    /// The blocks of its attribute of type `attribute_type`, if it has one: how
    /// one whose size varies is read. Not its type's default
    /// attribute, which is kept without its type
    /// ([`EntityRef::default_attribute`]).
    pub fn attribute_blocks(&self, attribute_type: AttributeType) -> Option<&'a [AttributeBlock]> {
        attribute_blocks(self.attributes, attribute_type)
    }

    /// Its type's default attribute, its type before its data as any
    /// attribute is given: if its type has one.
    #[inline]
    pub fn default_attribute(&self) -> Option<WholeAttribute> {
        self.default_kept().map(|(kept_default, default_blocks)| WholeAttribute::of_default(kept_default, default_blocks))
    }

    /// What is seen of its attribute of type `attribute_type`, in 64 bits
    /// ([`attribute_seen`]): what a compare of it is held against.
    #[inline]
    pub fn attribute_seen(&self, attribute_type: AttributeType) -> u64 {
        match self.default_if(attribute_type) {
            Some((kept_default, default_blocks)) => default_seen(kept_default, default_blocks),
            None => attribute_seen(attribute_blocks(self.attributes, attribute_type)),
        }
    }

    /// What the member of `byte_count` bytes from the byte `byte_offset` of its
    /// attribute of type `attribute_type` holds -- the offset counted from the
    /// attribute's type, the default attribute's too: none if it has
    /// no such attribute, or the member is not within one word of it.
    #[inline]
    pub fn attribute_member(&self, attribute_type: AttributeType, byte_offset: usize, byte_count: usize) -> Option<u64> {
        match self.default_if(attribute_type) {
            Some((_, default_blocks)) => attribute_bytes(default_blocks, byte_offset.checked_sub(8)?, byte_count),
            None => attribute_bytes(attribute_blocks(self.attributes, attribute_type)?, byte_offset, byte_count),
        }
    }

    /// All its blocks as its bucket keeps them: its default
    /// attribute's data, if its type has one, then its other
    /// attributes. What two entities are told apart by, and what is
    /// summed of one; not what an entity is given
    /// ([`EntityRef::default_attribute`]).
    #[inline]
    pub fn attribute_list(&self) -> &'a [AttributeBlock] {
        self.kept_blocks
    }
}
