//! What an attribute's data holds: a [`Layout`], fixed for its type,
//! and the attribute's constant typed by it ([`Attribute`])
//! (`docs/type_registry.md`, "Layouts").

use crate::type_ids::AttributeType;

/// Words in an attribute block: sixteen bytes.
pub const BLOCK_WORDS: usize = 2;
/// Bytes in an attribute block.
pub const BLOCK_BYTES: usize = BLOCK_WORDS * 8;
/// The most blocks an attribute takes: 256 bytes, its type among
/// them. What is larger is kept elsewhere, and the attribute says
/// where.
pub const ATTRIBUTE_BLOCKS_MOST: usize = 16;
/// The most words an attribute takes, its type the first.
pub const ATTRIBUTE_WORDS_MOST: usize = ATTRIBUTE_BLOCKS_MOST * BLOCK_WORDS;

/// The words of data an attribute of `blocks` blocks holds: all of
/// them but the first block's first, its type.
pub const fn data_words(blocks: usize) -> usize {
    blocks * BLOCK_WORDS - 1
}

/// A member of an attribute: one field of its layout, by where its
/// bytes are -- what an instruction reads or writes of an attribute,
/// and no more. Its bytes lie in one word, the lowest first: the same
/// on any machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// Its first byte, counted from the attribute's first: its type
    /// is the eight before any member.
    pub offset: u8,
    /// How many bytes it is: 1 to 8.
    pub bytes: u8,
}

impl Member {
    /// The member that is the whole word `word` of the data.
    pub const fn word(word: usize) -> Self {
        Self::within(word, 0, 8)
    }

    /// The member of `bytes` bytes from the byte `first` of the word
    /// `word` of the data.
    pub const fn within(word: usize, first: usize, bytes: usize) -> Self {
        assert!(bytes >= 1 && first + bytes <= 8 && (1 + word) < ATTRIBUTE_WORDS_MOST, "a member lies in one word of an attribute");
        Self { offset: ((1 + word) * 8 + first) as u8, bytes: bytes as u8 }
    }

    /// What it holds in `data`, an attribute's data words.
    pub const fn of(self, data: &[u64]) -> u64 {
        let (word, shift) = (self.offset as usize / 8 - 1, self.offset as u32 % 8 * 8);
        (data[word] >> shift) & (u64::MAX >> (64 - self.bytes as u32 * 8))
    }
}

/// How an attribute's data is laid out: what its words hold, member
/// by member, the same for every attribute of the type. Words, never
/// a machine's bytes, so the same on any machine.
pub trait Layout: Copy + PartialEq {
    /// Words of data an attribute so laid out holds: 1 to 31.
    const WORDS: usize;
    /// Blocks it takes, its type before its data: 1 to 16.
    const BLOCKS: usize = (1 + Self::WORDS).div_ceil(BLOCK_WORDS);
    /// Its members, each where its bytes are: what is written of it
    /// one at a time.
    const MEMBERS: &'static [Member];

    /// Writes its fields to `data`, the [`data_words`] of its blocks,
    /// all zero.
    fn write(self, data: &mut [u64]);

    /// Its fields, read from `data`, the [`data_words`] of its blocks.
    fn read(data: &[u64]) -> Self;
}

/// One number -- a tick, a count -- in the data's first word.
impl Layout for u64 {
    const WORDS: usize = 1;
    const MEMBERS: &'static [Member] = &[Member::word(0)];

    fn write(self, data: &mut [u64]) {
        data[0] = self;
    }

    fn read(data: &[u64]) -> Self {
        data[0]
    }
}

/// All a sheep keeps, in the three blocks of [`SHEEP_STATE`](crate::SHEEP_STATE):
/// a word a member, in the order written. A tick of 0 is none: the
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
    /// The neighbour it steps to while it roams, by its way: which
    /// of the nine cells about it.
    pub roaming_neighbour: u32,
}

impl Layout for SheepState {
    const WORDS: usize = 5;
    const MEMBERS: &'static [Member] = &[Member::word(0), Member::word(1), Member::word(2), Member::word(3), Member::within(4, 0, 4)];

    fn write(self, data: &mut [u64]) {
        data[..5].copy_from_slice(&[self.hungry_at, self.lamb_due_at, self.grown_at, self.roaming_until, self.roaming_neighbour as u64]);
    }

    fn read(data: &[u64]) -> Self {
        Self { hungry_at: data[0], lamb_due_at: data[1], grown_at: data[2], roaming_until: data[3], roaming_neighbour: data[4] as u32 }
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
