//! An attribute's blocks ([`AttributeBlock`]), and one attribute whole,
//! built where it is made ([`WholeAttribute`]) (`docs/entity_manager.md`,
//! "Attributes, a block each").

use crate::server_types::attribute_layouts::{Attribute, Layout, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS};
use crate::type_registry::AttributeType;

/// A block of an attribute: two words, sixteen bytes, aligned as
/// such. An attribute's first block has its header in its first word
/// (`AttributeType::header_word`): its type's number, and in the last
/// four bits how many blocks the attribute is.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributeBlock(pub [u64; BLOCK_WORDS]);

impl AttributeBlock {
    /// The block of `attribute`, whose layout takes one, holding
    /// `value`. A layout of more does not build here:
    /// [`WholeAttribute::holding_value`] makes any.
    #[inline]
    pub fn holding_value<L: Layout>(attribute: Attribute<L>, value: L) -> Self {
        const { assert!(L::BLOCK_COUNT == 1, "a layout of more blocks than one") };
        let mut made_block = Self([0; BLOCK_WORDS]);
        made_block.0[0] = attribute.attribute_type().header_word();
        value.write(&mut made_block.0[1..]);
        made_block
    }

    /// The type of the attribute it is the first block of.
    #[inline]
    pub fn attribute_type(&self) -> AttributeType {
        AttributeType::of_header_word(self.0[0])
    }

    /// Blocks the attribute it is the first block of takes: what
    /// the last four bits of its header say, one at least.
    #[inline]
    pub fn block_count(&self) -> usize {
        self.attribute_type().block_count()
    }
}

/// One attribute whole, laid out where it is made: its blocks, with
/// no room taken elsewhere for them -- the most an attribute is, 256
/// bytes. Read as its blocks.
#[derive(Clone, Copy)]
pub struct WholeAttribute {
    /// Its blocks, and nothing after them.
    block_array: [AttributeBlock; ATTRIBUTE_BLOCKS_MOST],
    /// How many of them are its.
    count: usize,
}

impl WholeAttribute {
    /// `attribute` holding `value`.
    #[inline]
    pub fn holding_value<L: Layout>(attribute: Attribute<L>, value: L) -> Self {
        const { assert!(L::BLOCK_COUNT <= ATTRIBUTE_BLOCKS_MOST, "a layout of more than 256 bytes") };
        let mut words = [0; ATTRIBUTE_WORDS_MOST];
        words[0] = attribute.attribute_type().header_word();
        value.write(&mut words[1..L::BLOCK_COUNT * BLOCK_WORDS]);
        let mut block_array = [AttributeBlock([0; BLOCK_WORDS]); ATTRIBUTE_BLOCKS_MOST];
        for (made_block, words) in block_array.iter_mut().zip(words.as_chunks::<BLOCK_WORDS>().0).take(L::BLOCK_COUNT) {
            made_block.0 = *words;
        }
        Self { block_array, count: L::BLOCK_COUNT }
    }
}

impl WholeAttribute {
    /// The attribute `block_array` holds in its first `count` blocks,
    /// its header first, the rest of it 0: one built block by block
    /// where it is read.
    ///
    /// # Panics
    /// If `count` is none, or more than [`ATTRIBUTE_BLOCKS_MOST`].
    #[inline]
    pub fn of_block_array(block_array: [AttributeBlock; ATTRIBUTE_BLOCKS_MOST], count: usize) -> Self {
        assert!((1..=ATTRIBUTE_BLOCKS_MOST).contains(&count), "an attribute of 1 to {ATTRIBUTE_BLOCKS_MOST} blocks");
        Self { block_array, count }
    }
}

impl std::ops::Deref for WholeAttribute {
    type Target = [AttributeBlock];

    #[inline]
    fn deref(&self) -> &[AttributeBlock] {
        &self.block_array[..self.count]
    }
}
