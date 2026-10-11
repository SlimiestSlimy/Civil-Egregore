//! What an attribute's data holds: a [`Layout`], fixed for its type,
//! and the attribute's constant typed by it ([`Attribute`])
//! (`docs/type_registry.md`, "Layouts").

use crate::type_ids::AttributeType;

/// Words in an attribute block: 64 bytes, a cache line.
pub const BLOCK_WORDS: usize = 8;

/// The words of data an attribute of `blocks` blocks holds: all of
/// them but the first block's first, its type.
pub const fn data_words(blocks: usize) -> usize {
    blocks * BLOCK_WORDS - 1
}

/// How an attribute's data is laid out: what its words hold, field by
/// field, the same for every attribute of the type. Words, never
/// bytes, so the same on any machine.
pub trait Layout: Copy + PartialEq {
    /// Blocks an attribute so laid out takes: 1 to 254.
    const BLOCKS: usize;

    /// Writes its fields to `data`, the [`data_words`] of its blocks,
    /// all zero.
    fn write(self, data: &mut [u64]);

    /// Its fields, read from `data`, the [`data_words`] of its blocks.
    fn read(data: &[u64]) -> Self;
}

/// One number -- a tick, a count -- in the data's first word.
impl Layout for u64 {
    const BLOCKS: usize = 1;

    fn write(self, data: &mut [u64]) {
        data[0] = self;
    }

    fn read(data: &[u64]) -> Self {
        data[0]
    }
}

/// All a sheep keeps, in the one block of [`SHEEP_STATE`](crate::SHEEP_STATE):
/// a word a field, in the order written. A tick of 0 is none: the
/// sheep is not in the middle of that.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SheepState {
    /// The tick it is next hungry at.
    pub hungry_at: u64,
    /// The tick its lamb is due at, if it is pregnant.
    pub lamb_due_at: u64,
    /// The tick it is grown at, if it is a lamb.
    pub grown_at: u64,
    /// The tick it roams until, if it is leaving thin pasture. Set
    /// once, at the meal, with the neighbour: a step on the way
    /// changes nothing.
    pub roaming_until: u64,
    /// The ID of the lamb it put beside it last tick, if it put one:
    /// pregnant still, until it has seen the lamb stand there.
    pub bearing: u64,
    /// The neighbour it steps to while it roams, by its way: which
    /// of the nine cells about it.
    pub roaming_neighbour: u32,
}

impl Layout for SheepState {
    const BLOCKS: usize = 1;

    fn write(self, data: &mut [u64]) {
        data[..6].copy_from_slice(&[self.hungry_at, self.lamb_due_at, self.grown_at, self.roaming_until, self.bearing, self.roaming_neighbour as u64]);
    }

    fn read(data: &[u64]) -> Self {
        Self { hungry_at: data[0], lamb_due_at: data[1], grown_at: data[2], roaming_until: data[3], bearing: data[4], roaming_neighbour: data[5] as u32 }
    }
}

/// An attribute, its layout in its type: read and written as its
/// fields, and one layout never read as another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribute<L: Layout> {
    /// Its type, which says how many blocks it takes.
    attribute_type: AttributeType,
    /// Its layout.
    layout: std::marker::PhantomData<L>,
}

impl<L: Layout> Attribute<L> {
    /// The attribute of number `number`, laid out as `L`.
    pub const fn new(number: u64) -> Self {
        Self { attribute_type: AttributeType::of_blocks(number, L::BLOCKS), layout: std::marker::PhantomData }
    }

    /// Its type: what its first block's first word holds, and what it
    /// is found among an entity's attributes by.
    pub const fn attribute_type(self) -> AttributeType {
        self.attribute_type
    }
}
