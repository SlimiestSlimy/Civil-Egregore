//! An entity's attributes: a run of 64-byte blocks sorted by type,
//! walked from the first, read and written as their layouts' fields
//! (`docs/entity_manager.md`, "Attributes, a block each").

use std::ops::Range;
pub use type_registry::{Attribute, AttributeType, Layout, BLOCK_WORDS};

/// A block of an attribute: eight words, a cache line. An attribute's
/// first block has its type in its first word; one whose size varies,
/// its block length in its second.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributeBlock(pub [u64; BLOCK_WORDS]);

impl AttributeBlock {
    /// The block of `attribute`, whose layout takes one, holding
    /// `value`. A layout of more does not build here:
    /// [`push_attribute`] makes any.
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

    /// Blocks the attribute it is the first block of takes: what its
    /// type says, or its block length -- one at least, that a walk
    /// always steps on.
    #[inline]
    pub fn blocks(&self) -> usize {
        self.kind().blocks().unwrap_or(self.0[1] as usize).max(1)
    }
}

/// Appends the blocks of `attribute` holding `value` to `blocks`.
#[inline]
pub fn push_attribute<L: Layout>(blocks: &mut Vec<AttributeBlock>, attribute: Attribute<L>, value: L) {
    if L::BLOCKS == 1 {
        blocks.push(AttributeBlock::holding_one(attribute, value));
        return;
    }
    // The data is one run of words over the blocks: the first block's seven, then eight a block.
    let mut words = vec![0; L::BLOCKS * BLOCK_WORDS];
    words[0] = attribute.attribute_type().0;
    value.write(&mut words[1..]);
    blocks.extend(words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block| AttributeBlock(block)));
}

impl AttributeBlock {
    /// [`AttributeBlock::holding`], the layout known to take one block
    /// by whoever calls.
    #[inline]
    fn holding_one<L: Layout>(attribute: Attribute<L>, value: L) -> Self {
        let mut block = Self([0; BLOCK_WORDS]);
        block.0[0] = attribute.attribute_type().0;
        value.write(&mut block.0[1..]);
        block
    }
}

/// Where the attribute of type `kind` is in `blocks`, an entity's,
/// sorted by type: its blocks, or where they would go. A walk from the
/// first, a step an attribute.
#[inline]
pub fn find_attribute(blocks: &[AttributeBlock], kind: AttributeType) -> Result<Range<usize>, usize> {
    let mut at = 0;
    while let Some(first) = blocks.get(at) {
        if first.kind() >= kind {
            // A block length that reaches past the entity's blocks is cut to them.
            return if first.kind() == kind { Ok(at..(at + first.blocks()).min(blocks.len())) } else { Err(at) };
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
    blocks.iter().flat_map(|block| block.0).fold(0xcbf2_9ce4_8422_2325, |sum, word| (sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29))
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
    let words: Vec<u64> = found.iter().flat_map(|block| block.0).collect();
    (words.len() == L::BLOCKS * BLOCK_WORDS).then(|| L::read(&words[1..]))
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
    if L::BLOCKS == 1 {
        set_attribute_blocks(blocks, &[AttributeBlock::holding_one(attribute, value)]);
    } else {
        let mut made = Vec::with_capacity(L::BLOCKS);
        push_attribute(&mut made, attribute, value);
        set_attribute_blocks(blocks, &made);
    }
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
        if !first.kind().is_an_attributes() || before.is_some_and(|before| before >= first.kind()) {
            return false;
        }
        (at, before) = (at + first.blocks(), Some(first.kind()));
    }
    at == blocks.len()
}
