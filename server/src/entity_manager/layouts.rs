//! What an attribute's data holds: a [`Layout`], fixed for its type,
//! and the attribute's constant typed by it ([`Attribute`])
//! (`docs/type_registry.md`, "Layouts").

use protocol::type_registry::{AttributeType, RegisteredType, Registration};

/// Words in an attribute block: sixteen bytes.
pub const BLOCK_WORDS: usize = 2;
/// Bytes in an attribute block.
pub const BLOCK_BYTES: usize = BLOCK_WORDS * 8;
/// The most blocks an attribute takes: 256 bytes, its type among
/// them. What is larger is kept elsewhere, and the attribute says
/// where.
pub const ATTRIBUTE_BLOCKS_MOST: usize = AttributeType::MOST_BLOCKS;
/// The most words an attribute takes, its type the first.
pub const ATTRIBUTE_WORDS_MOST: usize = ATTRIBUTE_BLOCKS_MOST * BLOCK_WORDS;

/// The words of data an attribute of `block_count` blocks holds: all of
/// them but the first block's first, its type.
pub const fn data_words(block_count: usize) -> usize {
    block_count * BLOCK_WORDS - 1
}

/// A member of an attribute: one field of its layout, by where its
/// bytes are -- what an instruction reads or writes of an attribute,
/// and no more. Its bytes lie in one word, the lowest first: the same
/// on any machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// Its first byte, counted from the attribute's first: its type
    /// is the eight before any member.
    pub byte_offset: u8,
    /// How many bytes it is: 1 to 8.
    pub byte_count: u8,
}

impl Member {
    /// The member that is the whole word `word` of the data.
    pub const fn word(word: usize) -> Self {
        Self::within_word(word, 0, 8)
    }

    /// The member of `byte_count` bytes from the byte `first_byte` of the word
    /// `word` of the data.
    pub const fn within_word(word: usize, first_byte: usize, byte_count: usize) -> Self {
        assert!(byte_count >= 1 && first_byte + byte_count <= 8 && (1 + word) < ATTRIBUTE_WORDS_MOST, "a member lies in one word of an attribute");
        Self { byte_offset: ((1 + word) * 8 + first_byte) as u8, byte_count: byte_count as u8 }
    }

    /// What it holds in `attribute_data`, an attribute's data words.
    pub const fn value_in(self, attribute_data: &[u64]) -> u64 {
        let (word, bit_shift) = (self.byte_offset as usize / 8 - 1, self.byte_offset as u32 % 8 * 8);
        (attribute_data[word] >> bit_shift) & (u64::MAX >> (64 - self.byte_count as u32 * 8))
    }
}

/// How an attribute's data is laid out: what its words hold, member
/// by member, the same for every attribute of the type. Words, never
/// a machine's bytes, so the same on any machine.
pub trait Layout: Copy + PartialEq {
    /// Words of data an attribute so laid out holds: 1 to 31.
    const WORDS: usize;
    /// Blocks it takes, its type before its data: 1 to 16.
    const BLOCK_COUNT: usize = (1 + Self::WORDS).div_ceil(BLOCK_WORDS);
    /// Its members, each where its bytes are: what is written of it
    /// one at a time.
    const MEMBERS: &'static [Member];

    /// Writes its fields to `attribute_data`, the [`data_words`] of its blocks,
    /// all zero.
    fn write(self, attribute_data: &mut [u64]);

    /// Its fields, read from `attribute_data`, the [`data_words`] of its blocks.
    fn read_data(attribute_data: &[u64]) -> Self;
}

/// One number -- a tick, a count -- in the data's first word.
impl Layout for u64 {
    const WORDS: usize = 1;
    const MEMBERS: &'static [Member] = &[Member::word(0)];

    fn write(self, attribute_data: &mut [u64]) {
        attribute_data[0] = self;
    }

    fn read_data(attribute_data: &[u64]) -> Self {
        attribute_data[0]
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
    /// The attribute `attribute_type` is, if it takes as many blocks as `L`.
    pub const fn of_attribute_type(attribute_type: AttributeType) -> Option<Self> {
        if attribute_type.block_count() == L::BLOCK_COUNT { Some(Self { attribute_type, layout: std::marker::PhantomData }) } else { None }
    }

    /// Its type: what its first block's first word holds, and what it
    /// is found among an entity's attributes by.
    pub const fn attribute_type(self) -> AttributeType {
        self.attribute_type
    }

    /// Its registration, named `name`: of a fixed size, its layout's.
    pub fn registration(name: &str) -> Registration {
        Registration::attribute(name, L::BLOCK_COUNT, L::WORDS)
    }

    /// The attribute `registered` is, if it is one laid out as `L`.
    pub fn registered(registered: &RegisteredType) -> Option<Self> {
        match registered {
            RegisteredType::Attribute { attribute_type, .. } => Self::of_attribute_type(*attribute_type),
            _ => None,
        }
    }
}
