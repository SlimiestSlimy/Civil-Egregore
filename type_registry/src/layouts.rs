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

/// Where a sheep leaving thin pasture roams, and until when: the
/// layout of [`ROAMING`](crate::ROAMING).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roaming {
    /// The tick it roams until: the data's first word.
    pub until: u64,
    /// The neighbour it steps to, its bit in the 3x3 cells about it:
    /// the second.
    pub neighbour: u32,
}

impl Layout for Roaming {
    const BLOCKS: usize = 1;

    fn write(self, data: &mut [u64]) {
        (data[0], data[1]) = (self.until, self.neighbour as u64);
    }

    fn read(data: &[u64]) -> Self {
        Self { until: data[0], neighbour: data[1] as u32 }
    }
}

/// The lamb a sheep has put beside it and not yet seen standing: the
/// layout of [`BEARING`](crate::BEARING).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bearing {
    /// The lamb's ID: the data's first word.
    pub lamb: u64,
    /// The neighbour it was put on, its bit in the 3x3 cells about
    /// its mother: the second.
    pub neighbour: u32,
}

impl Layout for Bearing {
    const BLOCKS: usize = 1;

    fn write(self, data: &mut [u64]) {
        (data[0], data[1]) = (self.lamb, self.neighbour as u64);
    }

    fn read(data: &[u64]) -> Self {
        Self { lamb: data[0], neighbour: data[1] as u32 }
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
