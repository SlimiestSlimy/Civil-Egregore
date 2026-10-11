//! An instruction: 24 bytes, the first two its opcode, the rest what
//! that opcode needs and no more. A rule's turn queues nothing else:
//! compares, writes, and the end of a compare, in three families by
//! what they are of -- plane and plane, plane and entity, entity and
//! entity -- and none knows what a rule means by it
//! (`docs/simulation.md`, "Instructions").

use chunk_storage::LayerType;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::AttributeType;

/// What an instruction does: its first two bytes. The high byte is
/// its family -- 0 neither compares nor writes the world, 1 plane and
/// plane, 2 plane and entity, 3 entity and entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub(crate) enum Opcode {
    /// The end of a compare: what follows is not under it.
    CompareEnd = 0x0000,
    /// Counts one under the number in `first`.
    Count = 0x0001,
    /// Holds if the layer `first` holds `small` at the cell.
    CompareCell = 0x0100,
    /// Makes the layer `first` hold `small` at the cell, counting
    /// under `second` if that is not [`NO_COUNT`].
    WriteCell = 0x0101,
    /// Holds if an entity may be made on the cell: nobody stands on
    /// it, it is nobody's name this tick, the collision plane does
    /// not hold it.
    CompareCellFree = 0x0200,
    /// Holds if the entity the tick found on the cell is there
    /// still, wherever it stands now, and of the type `first`.
    CompareEntity = 0x0205,
    /// Makes an entity of the type `first` on the cell, to wake at
    /// `second`, with no attribute, and holds the cell in the
    /// collision plane.
    MakeEntity = 0x0201,
    /// Moves the entity the tick found on the cell to the place
    /// `first` of the superchunk, the collision plane with it.
    MoveEntity = 0x0202,
    /// Removes the entity the tick found on the cell.
    RemoveEntity = 0x0203,
    /// Makes on the cell the entity crossing from the cell `first`
    /// of another superchunk, whole: its type, its wake and its
    /// attributes the blocks carried beside the queue from the one
    /// `second`'s high half numbers, as many as its low half and one.
    CrossEntity = 0x0204,
    /// Holds if the attribute numbered `small`, of the entity the
    /// tick found on the cell, sums to `first` -- or it has none, of
    /// `NO_ATTRIBUTE`.
    CompareAttribute = 0x0300,
    /// Holds if the word of that attribute at the offset is `first`.
    CompareAttributeWord = 0x0301,
    /// Holds if the two words from the offset are `first` and `second`.
    CompareAttributeWords = 0x0302,
    /// Writes `first` to the word of that attribute at the offset.
    WriteAttributeWord = 0x0303,
    /// Writes `first` and `second` to the two words from the offset:
    /// sixteen bytes, the most one instruction writes of an attribute.
    WriteAttributeWords = 0x0304,
    /// Gives the entity the attribute of the type `first`, `second`
    /// blocks of it, every word of its data 0 -- in place of the one
    /// it has, if it has one.
    AddAttribute = 0x0305,
    /// Takes the attribute of the type `first` from the entity.
    RemoveAttribute = 0x0306,
    /// Sets the tick the entity next wakes at to `first`: a write to
    /// its superchunk's timer wheel.
    WriteWake = 0x0307,
}

/// What a write counts under when it counts nothing.
pub(crate) const NO_COUNT: u64 = u64::MAX;
/// The bits of an instruction's `place` that are the cell's place in
/// its superchunk; those above are a word's offset in an attribute.
const PLACE_BITS: u32 = 20;
/// The most an attribute's number may be for an instruction to name
/// it: it has sixteen bits for it.
pub(crate) const ATTRIBUTE_NUMBER_MOST: u64 = u16::MAX as u64;
/// The most a word's offset in an attribute may be.
pub(crate) const WORD_OFFSET_MOST: usize = (1 << (u32::BITS - PLACE_BITS)) - 1;

/// One instruction (the module's docs): its opcode, sixteen bits and
/// thirty-two beside it, and two words -- each what the opcode says.
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub(crate) struct Instruction {
    /// What it does.
    pub(crate) opcode: Opcode,
    /// A cell's number, or an attribute's.
    pub(crate) small: u16,
    /// The cell's place in the superchunk it lands in, and, above
    /// that, a word's offset in an attribute.
    place: u32,
    /// The first word.
    pub(crate) first: u64,
    /// The second.
    pub(crate) second: u64,
}

// The size the instruction set is built to: no opcode has more.
const _: () = assert!(size_of::<Instruction>() == 24 && std::mem::offset_of!(Instruction, opcode) == 0 && coordinates::SUPERCHUNK_SIDE_CELLS as u64 * coordinates::SUPERCHUNK_SIDE_CELLS as u64 == 1 << PLACE_BITS);

impl Instruction {
    /// One of `opcode` on the cell `at`, all else 0.
    fn on(opcode: Opcode, at: CellIndex) -> Self {
        Self { opcode, small: 0, place: at.place_in_superchunk() as u32, first: 0, second: 0 }
    }

    /// One of `opcode` of the words from `offset` of the attribute
    /// `kind` of the entity on `at`.
    fn of_attribute(opcode: Opcode, at: CellIndex, kind: AttributeType, offset: usize) -> Self {
        assert!(kind.number() <= ATTRIBUTE_NUMBER_MOST && offset <= WORD_OFFSET_MOST, "attribute {} at word {offset}: past what an instruction names", kind.number());
        Self { small: kind.number() as u16, place: at.place_in_superchunk() as u32 | (offset as u32) << PLACE_BITS, ..Self::on(opcode, at) }
    }

    /// The end of a compare.
    pub(crate) fn compare_end() -> Self {
        Self { opcode: Opcode::CompareEnd, small: 0, place: 0, first: 0, second: 0 }
    }

    /// One counted under `number`.
    pub(crate) fn count(number: u32) -> Self {
        Self { opcode: Opcode::Count, first: u64::from(number), ..Self::compare_end() }
    }

    /// Whether `layer_type` holds `seen` at `at`.
    pub(crate) fn compare_cell(layer_type: LayerType, at: CellIndex, seen: u16) -> Self {
        Self { small: seen, first: layer_type.0, ..Self::on(Opcode::CompareCell, at) }
    }

    /// `layer_type` made to hold `value` at `at`, counted under `count`.
    pub(crate) fn write_cell(layer_type: LayerType, at: CellIndex, value: u16, count: u64) -> Self {
        Self { small: value, first: layer_type.0, second: count, ..Self::on(Opcode::WriteCell, at) }
    }

    /// Whether an entity may be made on `at`.
    pub(crate) fn compare_cell_free(at: CellIndex) -> Self {
        Self::on(Opcode::CompareCellFree, at)
    }

    /// Whether the entity of type `kind` the tick found on `at` is
    /// there still.
    pub(crate) fn compare_entity(at: CellIndex, kind: u64) -> Self {
        Self { first: kind, ..Self::on(Opcode::CompareEntity, at) }
    }

    /// An entity of type `kind` made on `at`, to wake at `wake`.
    pub(crate) fn make_entity(at: CellIndex, kind: u64, wake: u64) -> Self {
        Self { first: kind, second: wake, ..Self::on(Opcode::MakeEntity, at) }
    }

    /// The entity on `from` moved to `to`, a cell of its superchunk.
    pub(crate) fn move_entity(from: CellIndex, to: CellIndex) -> Self {
        debug_assert_eq!(from.superchunk(), to.superchunk(), "an entity moved to another superchunk: a crossing");
        Self { first: to.place_in_superchunk() as u64, ..Self::on(Opcode::MoveEntity, from) }
    }

    /// The entity on `at` removed.
    pub(crate) fn remove_entity(at: CellIndex) -> Self {
        Self::on(Opcode::RemoveEntity, at)
    }

    /// The entity crossing from `left` made on `at`, with the
    /// `count` blocks carried from `carried_first` on after its type
    /// and wake.
    pub(crate) fn cross_entity(at: CellIndex, left: CellIndex, carried_first: u32, count: u32) -> Self {
        Self { first: left.0, second: u64::from(carried_first) << 32 | u64::from(count), ..Self::on(Opcode::CrossEntity, at) }
    }

    /// Whether the attribute `kind` of the entity on `at` sums to `seen`.
    pub(crate) fn compare_attribute(at: CellIndex, kind: AttributeType, seen: u64) -> Self {
        Self { first: seen, ..Self::of_attribute(Opcode::CompareAttribute, at, kind, 0) }
    }

    /// Whether the words from `offset` of the attribute `kind` of the
    /// entity on `at` are `words`: one or two.
    pub(crate) fn compare_attribute_words(at: CellIndex, kind: AttributeType, offset: usize, words: &[u64]) -> Self {
        Self::of_words(Opcode::CompareAttributeWord, Opcode::CompareAttributeWords, at, kind, offset, words)
    }

    /// `words`, one or two, written from `offset` of the attribute
    /// `kind` of the entity on `at`.
    pub(crate) fn write_attribute_words(at: CellIndex, kind: AttributeType, offset: usize, words: &[u64]) -> Self {
        Self::of_words(Opcode::WriteAttributeWord, Opcode::WriteAttributeWords, at, kind, offset, words)
    }

    /// One of the opcode `of_one` or `of_two`, as `words` is one or two.
    fn of_words(of_one: Opcode, of_two: Opcode, at: CellIndex, kind: AttributeType, offset: usize, words: &[u64]) -> Self {
        match *words {
            [first] => Self { first, ..Self::of_attribute(of_one, at, kind, offset) },
            [first, second] => Self { first, second, ..Self::of_attribute(of_two, at, kind, offset) },
            _ => panic!("{} words in one instruction: it has room for two", words.len()),
        }
    }

    /// The entity on `at` given `blocks` blocks of the attribute `kind`.
    pub(crate) fn add_attribute(at: CellIndex, kind: AttributeType, blocks: usize) -> Self {
        Self { first: kind.0, second: blocks as u64, ..Self::on(Opcode::AddAttribute, at) }
    }

    /// The attribute `kind` taken from the entity on `at`.
    pub(crate) fn remove_attribute(at: CellIndex, kind: AttributeType) -> Self {
        Self { first: kind.0, ..Self::on(Opcode::RemoveAttribute, at) }
    }

    /// The entity on `at` to wake next at `wake`.
    pub(crate) fn write_wake(at: CellIndex, wake: u64) -> Self {
        Self { first: wake, ..Self::on(Opcode::WriteWake, at) }
    }

    /// The cell it is of, in `superchunk`, which it lands in.
    #[inline]
    pub(crate) fn at(&self, superchunk: SuperchunkIndex) -> CellIndex {
        CellIndex(superchunk.top_left().0 | u64::from(self.place & ((1 << PLACE_BITS) - 1)))
    }

    /// The cell of `superchunk` its first word is the place of.
    #[inline]
    pub(crate) fn to(&self, superchunk: SuperchunkIndex) -> CellIndex {
        CellIndex(superchunk.top_left().0 | self.first)
    }

    /// The offset of the word of an attribute it is of.
    #[inline]
    pub(crate) fn word_offset(&self) -> usize {
        (self.place >> PLACE_BITS) as usize
    }

    /// Whether it writes the world -- a plane, an entity, the wheel:
    /// what must lie under a compare.
    #[inline]
    pub(crate) fn is_a_write(&self) -> bool {
        !self.is_a_compare() && !matches!(self.opcode, Opcode::CompareEnd | Opcode::Count)
    }

    /// Whether it is a compare: what follows it is under it.
    #[inline]
    pub(crate) fn is_a_compare(&self) -> bool {
        matches!(self.opcode, Opcode::CompareCell | Opcode::CompareCellFree | Opcode::CompareEntity | Opcode::CompareAttribute | Opcode::CompareAttributeWord | Opcode::CompareAttributeWords)
    }
}
