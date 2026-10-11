//! What an entity is: a header -- its type, its cell, when it next
//! wakes -- and its attribute list, blocks that may grow or
//! shrink at run time. It has no ID: no two stand on one cell, so the
//! cell it stands on names it (`docs/entity_manager.md`,
//! "Instructions", An entity's name in a tick).

use crate::attributes::{attribute_blocks, attribute_bytes, attribute_seen, default_kept, default_seen, Attribute, AttributeBlock, AttributeType, DefaultKept, Layout, WholeAttribute, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS};
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
    pub(crate) kept: &'a [AttributeBlock],
}

impl<'a> EntityRef<'a> {
    /// Its type's default attribute as kept, and what kind it is, if
    /// its type has one: asked of the registry only then.
    #[inline]
    fn default_kept(&self) -> Option<(DefaultKept, &'a [AttributeBlock])> {
        let blocks = self.kept.len() - self.attributes.len();
        if blocks == 0 {
            return None;
        }
        Some((default_kept(self.header.kind)?, &self.kept[..blocks]))
    }

    /// Its type's default attribute as kept, if `kind` is it.
    #[inline]
    fn default_if(&self, kind: AttributeType) -> Option<(DefaultKept, &'a [AttributeBlock])> {
        self.default_kept().filter(|(default, _)| default.kind.identity() == kind.identity())
    }

    /// What its attribute `attribute` holds, if it has it: its
    /// type's default attribute always.
    #[inline]
    pub fn attribute<L: Layout>(&self, attribute: Attribute<L>) -> Option<L> {
        if let Some((default, kept)) = self.default_if(attribute.attribute_type()) {
            // Its words laid one after another where it is read: nothing taken for them.
            let mut words = [0; ATTRIBUTE_WORDS_MOST];
            for (words, block) in words.as_chunks_mut::<BLOCK_WORDS>().0.iter_mut().zip(kept) {
                *words = block.0;
            }
            return (default.words == L::WORDS).then(|| L::read(&words[..L::WORDS]));
        }
        crate::attributes::attribute(self.attributes, attribute)
    }

    /// The blocks of its attribute of type `kind`, if it has one: how
    /// one whose size varies is read. Not its type's default
    /// attribute, which is kept without its type
    /// ([`EntityRef::default_attribute`]).
    pub fn attribute_blocks(&self, kind: AttributeType) -> Option<&'a [AttributeBlock]> {
        attribute_blocks(self.attributes, kind)
    }

    /// Its type's default attribute, its type before its data as any
    /// attribute is given: if its type has one.
    #[inline]
    pub fn default_attribute(&self) -> Option<WholeAttribute> {
        self.default_kept().map(|(default, kept)| WholeAttribute::of_default(default, kept))
    }

    /// What is seen of its attribute of type `kind`, in 64 bits
    /// ([`attribute_seen`]): what a compare of it is held against.
    #[inline]
    pub fn attribute_seen(&self, kind: AttributeType) -> u64 {
        match self.default_if(kind) {
            Some((default, kept)) => default_seen(default, kept),
            None => attribute_seen(attribute_blocks(self.attributes, kind)),
        }
    }

    /// What the member of `bytes` bytes from the byte `offset` of its
    /// attribute of type `kind` holds -- the offset counted from the
    /// attribute's type, the default attribute's too: none if it has
    /// no such attribute, or the member is not within one word of it.
    #[inline]
    pub fn attribute_member(&self, kind: AttributeType, offset: usize, bytes: usize) -> Option<u64> {
        match self.default_if(kind) {
            Some((_, kept)) => attribute_bytes(kept, offset.checked_sub(8)?, bytes),
            None => attribute_bytes(attribute_blocks(self.attributes, kind)?, offset, bytes),
        }
    }

    /// All its blocks as its bucket keeps them: its default
    /// attribute's data, if its type has one, then its other
    /// attributes. What two entities are told apart by, and what is
    /// summed of one; not what an entity is given
    /// ([`EntityRef::default_attribute`]).
    #[inline]
    pub fn kept(&self) -> &'a [AttributeBlock] {
        self.kept
    }
}
