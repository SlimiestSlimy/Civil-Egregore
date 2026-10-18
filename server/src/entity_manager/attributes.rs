//! An entity's attributes: its attribute list, 16-byte blocks sorted by type,
//! walked from the first, read and written as their layouts' members
//! (`docs/entity_manager.md`, "Attributes, a block each").

use std::ops::Range;
pub use crate::entity_manager::layouts::{data_words, Attribute, Layout, Member, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS};
pub use protocol::type_registry::AttributeType;
use protocol::type_registry::{EntityType, FrozenTypes};

/// An entity type's default attribute as a bucket keeps it: first of
/// the entity's blocks, its data alone -- every entity of the type
/// has it and it is always as long, so it does not say its type
/// (`docs/entity_manager.md`, "The default attribute").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefaultKept {
    /// Its type, as it is named everywhere but where it is kept.
    pub attribute_type: AttributeType,
    /// The words its data is.
    pub words: usize,
    /// The blocks it is kept in: its data's, no word of type before.
    pub block_count: usize,
}

/// The world's types of entity with a default attribute, each with it
/// as kept: copied once from the types the world runs with, which never
/// change while it runs.
#[derive(Clone, Debug, Default)]
pub struct DefaultAttributes(Box<[(EntityType, DefaultKept)]>);

impl DefaultAttributes {
    /// The default attributes of `frozen_types`.
    pub fn of_types(frozen_types: &FrozenTypes) -> Self {
        Self(frozen_types.default_attributes().into_iter().map(|(entity_type, attribute_type, words)| (entity_type, DefaultKept { attribute_type, words, block_count: words.div_ceil(BLOCK_WORDS) })).collect())
    }

    /// The default attribute of entities of type `entity_type`, if the
    /// type has one.
    #[inline]
    pub fn of(&self, entity_type: EntityType) -> Option<DefaultKept> {
        self.0.iter().find(|(kept_type, _)| *kept_type == entity_type).map(|(_, kept_default)| *kept_default)
    }
}

/// The word at `word` of `source_blocks`, one after another.
#[inline]
fn word_of(source_blocks: &[AttributeBlock], word: usize) -> u64 {
    source_blocks[word / BLOCK_WORDS].0[word % BLOCK_WORDS]
}

/// Appends to `kept_blocks` the blocks of an entity whose type's default
/// attribute is `kept_default`, if any, given `attributes`, sorted by
/// type, as a bucket keeps them: its default attribute first, without
/// its type -- zeroes if it is not among those given -- then the others
/// as they are.
pub fn append_kept(kept_default: Option<DefaultKept>, attributes: &[AttributeBlock], kept_blocks: &mut Vec<AttributeBlock>) {
    let Some(kept_default) = kept_default else {
        return kept_blocks.extend_from_slice(attributes);
    };
    let first_default_block = kept_blocks.len();
    kept_blocks.resize(first_default_block + kept_default.block_count, AttributeBlock([0; BLOCK_WORDS]));
    for attribute in each_attribute(attributes) {
        if attribute[0].attribute_type().type_identity() == kept_default.attribute_type.type_identity() {
            write_default(&mut kept_blocks[first_default_block..first_default_block + kept_default.block_count], kept_default, attribute);
        } else {
            kept_blocks.extend_from_slice(attribute);
        }
    }
}

/// Writes over the default attribute `default_blocks` -- a bucket's, of the
/// kind `kept_default` -- the data of `given_attribute`, the same attribute with its
/// type before it: whether `given_attribute` is as long as that takes.
pub(crate) fn write_default(default_blocks: &mut [AttributeBlock], kept_default: DefaultKept, given_attribute: &[AttributeBlock]) -> bool {
    if given_attribute.len() != kept_default.attribute_type.block_count() {
        return false;
    }
    for word in 0..kept_default.words {
        default_blocks[word / BLOCK_WORDS].0[word % BLOCK_WORDS] = word_of(given_attribute, word + 1);
    }
    true
}

/// What is seen of the default attribute `default_blocks` -- a bucket's, of
/// the kind `kept_default` -- in 64 bits: [`attribute_seen`] of it with
/// its type before it, summed where it lies, nothing laid out.
#[inline]
pub(crate) fn default_seen(kept_default: DefaultKept, default_blocks: &[AttributeBlock]) -> u64 {
    let data_words = default_blocks.iter().flat_map(|kept_block| kept_block.0).take(kept_default.words);
    // As long as it is given: its type, its data, and nothing to the end of its last block.
    let words = std::iter::once(kept_default.attribute_type.0).chain(data_words).chain(std::iter::repeat(0)).take(kept_default.attribute_type.block_count() * BLOCK_WORDS);
    words.fold(0xcbf2_9ce4_8422_2325, |running_sum: u64, word| (running_sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29)).max(NO_ATTRIBUTE + 1)
}

/// Whether `attributes` are ones an entity whose type's default attribute
/// is `kept_default`, if any, may be given: whole ones, each of a type
/// that is an attribute's, each type once, sorted by type but for the
/// type's default attribute, which may come anywhere among them.
pub fn may_be_given(kept_default: Option<DefaultKept>, attributes: &[AttributeBlock]) -> bool {
    let Some(kept_default) = kept_default else {
        return is_attribute_list(attributes);
    };
    let mut kept_blocks = Vec::new();
    append_kept(Some(kept_default), attributes, &mut kept_blocks);
    let default_lengths: Vec<usize> = each_attribute(attributes).filter(|attribute| attribute[0].attribute_type().type_identity() == kept_default.attribute_type.type_identity()).map(<[_]>::len).collect();
    is_attribute_list(&kept_blocks[kept_default.block_count..]) && default_lengths.len() <= 1 && default_lengths.iter().all(|&default_length| default_length == kept_default.attribute_type.block_count())
}

/// A block of an attribute: two words, sixteen bytes, aligned as
/// such. An attribute's first block has its type in its first word,
/// whose last four bits say how many blocks the attribute is.
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
        made_block.0[0] = attribute.attribute_type().0;
        value.write(&mut made_block.0[1..]);
        made_block
    }

    /// The type of the attribute it is the first block of.
    #[inline]
    pub fn attribute_type(&self) -> AttributeType {
        AttributeType(self.0[0])
    }

    /// Blocks the attribute it is the first block of takes: what
    /// the last four bits of its type say, one at least.
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
        words[0] = attribute.attribute_type().0;
        value.write(&mut words[1..L::BLOCK_COUNT * BLOCK_WORDS]);
        let mut block_array = [AttributeBlock([0; BLOCK_WORDS]); ATTRIBUTE_BLOCKS_MOST];
        for (made_block, words) in block_array.iter_mut().zip(words.as_chunks::<BLOCK_WORDS>().0).take(L::BLOCK_COUNT) {
            made_block.0 = *words;
        }
        Self { block_array, count: L::BLOCK_COUNT }
    }
}

impl WholeAttribute {
    /// The default attribute `default_blocks` -- a bucket's, of the kind
    /// `kept_default` -- with its type before it, as it is given and
    /// compared everywhere else.
    #[inline]
    pub(crate) fn of_default(kept_default: DefaultKept, default_blocks: &[AttributeBlock]) -> Self {
        let (mut block_array, count) = ([AttributeBlock([0; BLOCK_WORDS]); ATTRIBUTE_BLOCKS_MOST], kept_default.attribute_type.block_count());
        block_array[0].0[0] = kept_default.attribute_type.0;
        for word in 0..kept_default.words {
            block_array[(word + 1) / BLOCK_WORDS].0[(word + 1) % BLOCK_WORDS] = word_of(default_blocks, word);
        }
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

/// Appends the blocks of `attribute` holding `value` to `attribute_list`.
#[inline]
pub fn push_attribute<L: Layout>(attribute_list: &mut Vec<AttributeBlock>, attribute: Attribute<L>, value: L) {
    attribute_list.extend_from_slice(&WholeAttribute::holding_value(attribute, value));
}

/// What the member of `byte_count` bytes from the byte `byte_offset` of the
/// attribute `whole_attribute` is holds: none if it does not lie in one word
/// of it.
#[inline]
pub fn attribute_bytes(whole_attribute: &[AttributeBlock], byte_offset: usize, byte_count: usize) -> Option<u64> {
    let (word, first_byte) = (byte_offset / 8, byte_offset % 8);
    if byte_count == 0 || first_byte + byte_count > 8 {
        return None;
    }
    let held_word = whole_attribute.get(word / BLOCK_WORDS)?.0[word % BLOCK_WORDS];
    Some((held_word >> (first_byte * 8)) & (u64::MAX >> (64 - byte_count * 8)))
}

/// Writes the low `byte_count` bytes of `value` over the member from the
/// byte `byte_offset` of the attribute `whole_attribute` is, in place: whether it
/// was written -- not over its type, nor if the member does not lie
/// in one word of it.
#[inline]
pub fn write_attribute_bytes(whole_attribute: &mut [AttributeBlock], byte_offset: usize, byte_count: usize, value: u64) -> bool {
    byte_offset >= 8 && write_bytes(whole_attribute, byte_offset, byte_count, value)
}

/// Writes the low `byte_count` bytes of `value` over those from the byte
/// `byte_offset` of `written_blocks`, in place, whatever they are: whether it was
/// written -- not if they do not lie in one word of them.
#[inline]
pub(crate) fn write_bytes(written_blocks: &mut [AttributeBlock], byte_offset: usize, byte_count: usize, value: u64) -> bool {
    let (word, first_byte) = (byte_offset / 8, byte_offset % 8);
    let Some(held_block) = written_blocks.get_mut(word / BLOCK_WORDS) else {
        return false;
    };
    if byte_count == 0 || first_byte + byte_count > 8 {
        return false;
    }
    let mask = (u64::MAX >> (64 - byte_count * 8)) << (first_byte * 8);
    let held_word = &mut held_block.0[word % BLOCK_WORDS];
    *held_word = (*held_word & !mask) | ((value << (first_byte * 8)) & mask);
    true
}

/// Where the attribute of type `attribute_type` is in `attribute_list`, an entity's,
/// sorted by type: its blocks, or where they would go. A walk from the
/// first, a step an attribute.
#[inline]
pub fn find_attribute(attribute_list: &[AttributeBlock], attribute_type: AttributeType) -> Result<Range<usize>, usize> {
    let mut block_index = 0;
    while let Some(first_block) = attribute_list.get(block_index) {
        // By what tells types apart: an attribute whose size varies says its own length in its type.
        if first_block.attribute_type().type_identity() >= attribute_type.type_identity() {
            // A block length that reaches past the entity's blocks is cut to them.
            return if first_block.attribute_type().type_identity() == attribute_type.type_identity() { Ok(block_index..(block_index + first_block.block_count()).min(attribute_list.len())) } else { Err(block_index) };
        }
        block_index += first_block.block_count();
    }
    Err(attribute_list.len())
}

/// A sum of `summed_blocks`, the same for the same blocks in the same order:
/// what two attribute lists are told apart by where one of them is
/// not at hand -- in another superchunk, or queued with a compare. Two
/// that differ sum the same once in 2^64 or so.
pub fn blocks_sum(summed_blocks: &[AttributeBlock]) -> u64 {
    summed_blocks.iter().fold(0xcbf2_9ce4_8422_2325, |list_sum, summed_block| summed_block.0.iter().fold(list_sum, |running_sum, &word| (running_sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29)))
}

/// [`blocks_sum`] of the blocks `words` are, one after another.
#[inline]
pub fn words_sum(words: &[u64]) -> u64 {
    words.iter().fold(0xcbf2_9ce4_8422_2325, |running_sum, &word| (running_sum ^ word).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(29))
}

/// What is seen of an entity that has no such attribute
/// ([`attribute_seen`]).
pub const NO_ATTRIBUTE: u64 = 0;

/// What a rule saw of one attribute, in 64 bits: [`NO_ATTRIBUTE`] of
/// none, else the sum of its blocks ([`blocks_sum`]) -- what a
/// compare carries of it, and what it is held against as it is
/// applied. Never a block.
pub fn attribute_seen(maybe_attribute: Option<&[AttributeBlock]>) -> u64 {
    maybe_attribute.map_or(NO_ATTRIBUTE, |whole_attribute| blocks_sum(whole_attribute).max(NO_ATTRIBUTE + 1))
}

/// The attributes in `attribute_list`, sorted by type, one after another: the
/// blocks of each.
pub fn each_attribute(attribute_list: &[AttributeBlock]) -> impl Iterator<Item = &[AttributeBlock]> {
    let mut block_index = 0;
    std::iter::from_fn(move || {
        let first_block = attribute_list.get(block_index)?;
        let attribute = &attribute_list[block_index..(block_index + first_block.block_count()).min(attribute_list.len())];
        block_index += attribute.len();
        Some(attribute)
    })
}

/// The blocks of the attribute of type `attribute_type` in `attribute_list`, sorted by
/// type, if there is one.
#[inline]
pub fn attribute_blocks(attribute_list: &[AttributeBlock], attribute_type: AttributeType) -> Option<&[AttributeBlock]> {
    find_attribute(attribute_list, attribute_type).ok().map(|found_range| &attribute_list[found_range])
}

/// What `attribute` holds in `attribute_list`, sorted by type, if it is there.
#[inline]
pub fn attribute<L: Layout>(attribute_list: &[AttributeBlock], attribute: Attribute<L>) -> Option<L> {
    let found_blocks = attribute_blocks(attribute_list, attribute.attribute_type())?;
    if L::BLOCK_COUNT == 1 {
        return Some(L::read_data(&found_blocks[0].0[1..]));
    }
    if found_blocks.len() != L::BLOCK_COUNT {
        return None;
    }
    // Its words laid one after another where it is read: nothing taken for them.
    let mut words = [0; ATTRIBUTE_WORDS_MOST];
    for (words, found_block) in words.as_chunks_mut::<BLOCK_WORDS>().0.iter_mut().zip(found_blocks) {
        *words = found_block.0;
    }
    Some(L::read_data(&words[1..L::BLOCK_COUNT * BLOCK_WORDS]))
}

/// Sets the attribute `attribute` is the blocks of in `attribute_list`, sorted
/// by type: in place of the one of its type, or added.
pub fn set_attribute_blocks(attribute_list: &mut Vec<AttributeBlock>, attribute: &[AttributeBlock]) {
    debug_assert!(attribute.first().is_some_and(|first_block| first_block.block_count() == attribute.len()), "an attribute as long as its type or its block length says");
    match find_attribute(attribute_list, attribute[0].attribute_type()) {
        Ok(found_range) if found_range.len() == attribute.len() => attribute_list[found_range].copy_from_slice(attribute),
        Ok(found_range) => _ = attribute_list.splice(found_range, attribute.iter().copied()),
        Err(insert_index) => _ = attribute_list.splice(insert_index..insert_index, attribute.iter().copied()),
    }
}

/// Sets `attribute` in `attribute_list`, sorted by type, to `value`: added if
/// it was not there.
#[inline]
pub fn set_attribute<L: Layout>(attribute_list: &mut Vec<AttributeBlock>, attribute: Attribute<L>, value: L) {
    set_attribute_blocks(attribute_list, &WholeAttribute::holding_value(attribute, value));
}

/// Removes the attribute of type `attribute_type` from `attribute_list`, sorted by type:
/// whether it was there.
pub fn remove_attribute(attribute_list: &mut Vec<AttributeBlock>, attribute_type: AttributeType) -> bool {
    let search_result = find_attribute(attribute_list, attribute_type);
    if let Ok(found_range) = &search_result {
        attribute_list.drain(found_range.clone());
    }
    search_result.is_ok()
}

/// Whether `candidate_blocks` are an entity's attributes: whole ones, each of
/// a type that is an attribute's, sorted by type, each type once.
pub fn is_attribute_list(candidate_blocks: &[AttributeBlock]) -> bool {
    let (mut block_index, mut previous_identity) = (0, None);
    while let Some(first_block) = candidate_blocks.get(block_index) {
        if !first_block.attribute_type().is_an_attributes() || previous_identity.is_some_and(|last_identity| last_identity >= first_block.attribute_type().type_identity()) {
            return false;
        }
        (block_index, previous_identity) = (block_index + first_block.block_count(), Some(first_block.attribute_type().type_identity()));
    }
    block_index == candidate_blocks.len()
}
