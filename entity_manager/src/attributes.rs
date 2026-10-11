//! An entity's attributes: a run of 16-byte blocks sorted by type,
//! walked from the first, read and written as their layouts' members
//! (`docs/entity_manager.md`, "Attributes, a block each").

use std::ops::Range;
pub use type_registry::{Attribute, AttributeType, Layout, Member, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS};

/// A block of an attribute: two words, sixteen bytes, aligned as
/// such. An attribute's first block has its type in its first word,
/// whose last four bits say how many blocks the attribute is.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributeBlock(pub [u64; BLOCK_WORDS]);

impl AttributeBlock {
    /// The block of `attribute`, whose layout takes one, holding
    /// `value`. A layout of more does not build here:
    /// [`WholeAttribute::holding`] makes any.
    #[inline]
    pub fn holding<L: Layout>(attribute: Attribute<L>, value: L) -> Self {
        const { assert!(L::BLOCKS == 1, "a layout of more blocks than one") };
        let mut block = Self([0; BLOCK_WORDS]);
        block.0[0] = attribute.attribute_type().0;
        value.write(&mut block.0[1..]);
        block
    }

    /// The type of the attribute it is the first block of.
    #[inline]
    pub fn kind(&self) -> AttributeType {
        AttributeType(self.0[0])
    }

    /// Blocks the attribute it is the first block of takes: what
    /// the last four bits of its type say, one at least.
    #[inline]
    pub fn blocks(&self) -> usize {
        self.kind().blocks()
    }
}

/// One attribute whole, laid out where it is made: its blocks, with
/// no room taken elsewhere for them -- the most an attribute is, 256
/// bytes. Read as its blocks.
#[derive(Clone, Copy)]
pub struct WholeAttribute {
    /// Its blocks, and nothing after them.
    blocks: [AttributeBlock; ATTRIBUTE_BLOCKS_MOST],
    /// How many of them are its.
    count: usize,
}

impl WholeAttribute {
    /// `attribute` holding `value`.
    #[inline]
    pub fn holding<L: Layout>(attribute: Attribute<L>, value: L) -> Self {
        const { assert!(L::BLOCKS <= ATTRIBUTE_BLOCKS_MOST, "a layout of more than 256 bytes") };
        let mut words = [0; ATTRIBUTE_WORDS_MOST];
        words[0] = attribute.attribute_type().0;
        value.write(&mut words[1..L::BLOCKS * BLOCK_WORDS]);
        let mut blocks = [AttributeBlock([0; BLOCK_WORDS]); ATTRIBUTE_BLOCKS_MOST];
        for (block, words) in blocks.iter_mut().zip(words.as_chunks::<BLOCK_WORDS>().0).take(L::BLOCKS) {
            block.0 = *words;
        }
        Self { blocks, count: L::BLOCKS }
    }
}

impl std::ops::Deref for WholeAttribute {
    type Target = [AttributeBlock];

    #[inline]
    fn deref(&self) -> &[AttributeBlock] {
        &self.blocks[..self.count]
    }
}

/// Appends the blocks of `attribute` holding `value` to `blocks`.
#[inline]
pub fn push_attribute<L: Layout>(blocks: &mut Vec<AttributeBlock>, attribute: Attribute<L>, value: L) {
    blocks.extend_from_slice(&WholeAttribute::holding(attribute, value));
}

/// What the member of `bytes` bytes from the byte `offset` of the
/// attribute `blocks` is holds: none if it does not lie in one word
/// of it.
#[inline]
pub fn attribute_bytes(blocks: &[AttributeBlock], offset: usize, bytes: usize) -> Option<u64> {
    let (word, first) = (offset / 8, offset % 8);
    if bytes == 0 || first + bytes > 8 {
        return None;
    }
    let held = blocks.get(word / BLOCK_WORDS)?.0[word % BLOCK_WORDS];
    Some((held >> (first * 8)) & (u64::MAX >> (64 - bytes * 8)))
}

/// Writes the low `bytes` bytes of `value` over the member from the
/// byte `offset` of the attribute `blocks` is, in place: whether it
/// was written -- not over its type, nor if the member does not lie
/// in one word of it.
#[inline]
pub fn write_attribute_bytes(blocks: &mut [AttributeBlock], offset: usize, bytes: usize, value: u64) -> bool {
    let (word, first) = (offset / 8, offset % 8);
    let Some(block) = blocks.get_mut(word / BLOCK_WORDS) else {
        return false;
    };
    if word == 0 || bytes == 0 || first + bytes > 8 {
        return false;
    }
    let mask = (u64::MAX >> (64 - bytes * 8)) << (first * 8);
    let held = &mut block.0[word % BLOCK_WORDS];
    *held = (*held & !mask) | ((value << (first * 8)) & mask);
    true
}

/// Where the attribute of type `kind` is in `blocks`, an entity's,
/// sorted by type: its blocks, or where they would go. A walk from the
/// first, a step an attribute.
#[inline]
pub fn find_attribute(blocks: &[AttributeBlock], kind: AttributeType) -> Result<Range<usize>, usize> {
    let mut at = 0;
    while let Some(first) = blocks.get(at) {
        // By what tells types apart: an attribute whose size varies says its own length in its type.
        if first.kind().identity() >= kind.identity() {
            // A block length that reaches past the entity's blocks is cut to them.
            return if first.kind().identity() == kind.identity() { Ok(at..(at + first.blocks()).min(blocks.len())) } else { Err(at) };
        }
        at += first.blocks();
    }
    Err(blocks.len())
}

/// A sum of `blocks`, the same for the same blocks in the same order:
/// what two runs of attributes are told apart by where one of them is
/// not at hand -- in another superchunk, or queued with a compare. Two
/// that differ sum the same once in 2^64 or so.
pub fn blocks_sum(blocks: &[AttributeBlock]) -> u64 {
    blocks.iter().fold(0xcbf2_9ce4_8422_2325, |sum, block| block.0.iter().fold(sum, |sum, &word| (sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29)))
}

/// [`blocks_sum`] of the blocks `words` are, one after another.
#[inline]
pub fn words_sum(words: &[u64]) -> u64 {
    words.iter().fold(0xcbf2_9ce4_8422_2325, |sum, &word| (sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29))
}

/// What is seen of an entity that has no such attribute
/// ([`attribute_seen`]).
pub const NO_ATTRIBUTE: u64 = 0;

/// What a rule saw of one attribute, in 64 bits: [`NO_ATTRIBUTE`] of
/// none, else the sum of its blocks ([`blocks_sum`]) -- what a
/// compare carries of it, and what it is held against as it is
/// applied. Never a block.
pub fn attribute_seen(blocks: Option<&[AttributeBlock]>) -> u64 {
    blocks.map_or(NO_ATTRIBUTE, |blocks| blocks_sum(blocks).max(NO_ATTRIBUTE + 1))
}

/// The attributes in `blocks`, sorted by type, one after another: the
/// blocks of each.
pub fn each_attribute(blocks: &[AttributeBlock]) -> impl Iterator<Item = &[AttributeBlock]> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let first = blocks.get(at)?;
        let attribute = &blocks[at..(at + first.blocks()).min(blocks.len())];
        at += attribute.len();
        Some(attribute)
    })
}

/// The blocks of the attribute of type `kind` in `blocks`, sorted by
/// type, if there is one.
#[inline]
pub fn attribute_blocks(blocks: &[AttributeBlock], kind: AttributeType) -> Option<&[AttributeBlock]> {
    find_attribute(blocks, kind).ok().map(|found| &blocks[found])
}

/// What `attribute` holds in `blocks`, sorted by type, if it is there.
#[inline]
pub fn attribute<L: Layout>(blocks: &[AttributeBlock], attribute: Attribute<L>) -> Option<L> {
    let found = attribute_blocks(blocks, attribute.attribute_type())?;
    if L::BLOCKS == 1 {
        return Some(L::read(&found[0].0[1..]));
    }
    if found.len() != L::BLOCKS {
        return None;
    }
    // Its words laid one after another where it is read: nothing taken for them.
    let mut words = [0; ATTRIBUTE_WORDS_MOST];
    for (words, block) in words.as_chunks_mut::<BLOCK_WORDS>().0.iter_mut().zip(found) {
        *words = block.0;
    }
    Some(L::read(&words[1..L::BLOCKS * BLOCK_WORDS]))
}

/// Sets the attribute `attribute` is the blocks of in `blocks`, sorted
/// by type: in place of the one of its type, or added.
pub fn set_attribute_blocks(blocks: &mut Vec<AttributeBlock>, attribute: &[AttributeBlock]) {
    debug_assert!(attribute.first().is_some_and(|first| first.blocks() == attribute.len()), "an attribute as long as its type or its block length says");
    match find_attribute(blocks, attribute[0].kind()) {
        Ok(found) if found.len() == attribute.len() => blocks[found].copy_from_slice(attribute),
        Ok(found) => _ = blocks.splice(found, attribute.iter().copied()),
        Err(at) => _ = blocks.splice(at..at, attribute.iter().copied()),
    }
}

/// Sets `attribute` in `blocks`, sorted by type, to `value`: added if
/// it was not there.
#[inline]
pub fn set_attribute<L: Layout>(blocks: &mut Vec<AttributeBlock>, attribute: Attribute<L>, value: L) {
    set_attribute_blocks(blocks, &WholeAttribute::holding(attribute, value));
}

/// Removes the attribute of type `kind` from `blocks`, sorted by type:
/// whether it was there.
pub fn remove_attribute(blocks: &mut Vec<AttributeBlock>, kind: AttributeType) -> bool {
    let found = find_attribute(blocks, kind);
    if let Ok(found) = &found {
        blocks.drain(found.clone());
    }
    found.is_ok()
}

/// Whether `blocks` are an entity's attributes: whole ones, each of
/// a type that is an attribute's, sorted by type, each type once.
pub fn sorted(blocks: &[AttributeBlock]) -> bool {
    let (mut at, mut before) = (0, None);
    while let Some(first) = blocks.get(at) {
        if !first.kind().is_an_attributes() || before.is_some_and(|before| before >= first.kind().identity()) {
            return false;
        }
        (at, before) = (at + first.blocks(), Some(first.kind().identity()));
    }
    at == blocks.len()
}
