//! An instruction: 24 bytes, the first two its opcode, the rest what
//! that opcode needs and no more. A rule's turn queues nothing else:
//! compares, writes, and the end of a compare, in three families by
//! what they are of -- plane and plane, plane and entity, entity and
//! entity -- and none knows what a rule means by it
//! (`docs/simulation.md`, "Instructions").

use chunk_storage::LayerType;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::{AttributeType, Member};

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
    /// Holds if the attribute of the type `second`, of the entity the
    /// tick found on the cell, sums to `first` -- or it has none, of
    /// `NO_ATTRIBUTE`.
    CompareAttribute = 0x0300,
    /// Holds if a member of the attribute of the type `first` holds
    /// `second`: the member `small` says, its first byte in its low
    /// byte, how many bytes it is in its high.
    CompareAttributeMember = 0x0301,
    /// Writes `second` to a member of the attribute of the type
    /// `first`, the member said likewise: its bytes and no more, eight
    /// at the most.
    WriteAttributeMember = 0x0303,
    /// Gives the entity an attribute whole, in place of the one of
    /// its type if it has one: the blocks carried beside the queue
    /// from the one `second`'s high half numbers, as many as its low
    /// half. A write in bulk.
    WriteAttribute = 0x0304,
    /// Takes the attribute of the type `first` from the entity.
    RemoveAttribute = 0x0306,
    /// Sets the tick the entity next wakes at to `first`: a write to
    /// its superchunk's timer wheel.
    WriteWake = 0x0307,
}

/// What a write counts under when it counts nothing.
pub(crate) const NO_COUNT: u32 = u32::MAX;
/// The bits of an instruction's `place` that are the cell's place in
/// its superchunk; the four above say which superchunk it lands in,
/// of its author's and the eight about that, by outbox slot.
const PLACE_BITS: u32 = 20;

/// One instruction (the module's docs): its opcode, sixteen bits and
/// thirty-two beside it, and two words -- each what the opcode says.
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub(crate) struct Instruction {
    /// What it does.
    pub(crate) opcode: Opcode,
    /// Which member of an attribute.
    pub(crate) small: u16,
    /// The cell's place in the superchunk it lands in, and above it
    /// the outbox slot of that superchunk.
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

    /// The end of a compare.
    pub(crate) fn compare_end() -> Self {
        Self { opcode: Opcode::CompareEnd, small: 0, place: 0, first: 0, second: 0 }
    }

    /// One counted under `number`.
    pub(crate) fn count_under(number: u32) -> Self {
        Self { opcode: Opcode::Count, first: u64::from(number), ..Self::compare_end() }
    }

    /// Whether `layer_type` holds `seen` at `at`.
    pub(crate) fn compare_cell(layer_type: LayerType, at: CellIndex, seen: u32) -> Self {
        Self { first: layer_type.0, second: u64::from(seen), ..Self::on(Opcode::CompareCell, at) }
    }

    /// `layer_type` made to hold `value` at `at`, counted under
    /// `count`, or under nothing ([`NO_COUNT`]): the value the second
    /// word's high half, the count its low.
    pub(crate) fn write_cell(layer_type: LayerType, at: CellIndex, value: u32, count: u32) -> Self {
        Self { first: layer_type.0, second: u64::from(value) << 32 | u64::from(count), ..Self::on(Opcode::WriteCell, at) }
    }

    /// The number a write of a cell makes its cell hold.
    pub(crate) fn value_written(&self) -> u32 {
        (self.second >> 32) as u32
    }

    /// What a write of a cell counts under: [`NO_COUNT`] for nothing.
    pub(crate) fn counted_under(&self) -> u32 {
        self.second as u32
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

    /// The entity the tick found on `from` moved to `to`, a cell of
    /// another superchunk: a crossing. It lands where it stood; the
    /// cell it comes to, whole, is beside it.
    pub(crate) fn cross_entity(from: CellIndex, to: CellIndex) -> Self {
        debug_assert_ne!(from.superchunk(), to.superchunk(), "a crossing within a superchunk: a move");
        Self { first: to.0, ..Self::on(Opcode::CrossEntity, from) }
    }

    /// Whether the attribute `kind` of the entity on `at` sums to `seen`.
    pub(crate) fn compare_attribute(at: CellIndex, kind: AttributeType, seen: u64) -> Self {
        Self { first: seen, second: kind.0, ..Self::on(Opcode::CompareAttribute, at) }
    }

    /// Whether `member` of the attribute `kind` of the entity on `at`
    /// holds `seen`.
    pub(crate) fn compare_attribute_member(at: CellIndex, kind: AttributeType, member: Member, seen: u64) -> Self {
        Self { small: u16::from(member.offset) | u16::from(member.bytes) << 8, first: kind.0, second: seen, ..Self::on(Opcode::CompareAttributeMember, at) }
    }

    /// `value` written to `member` of the attribute `kind` of the
    /// entity on `at`.
    pub(crate) fn write_attribute_member(at: CellIndex, kind: AttributeType, member: Member, value: u64) -> Self {
        Self { opcode: Opcode::WriteAttributeMember, ..Self::compare_attribute_member(at, kind, member, value) }
    }

    /// The entity on `at` given the attribute that is the `count`
    /// blocks carried from `carried_first` on.
    pub(crate) fn write_attribute(at: CellIndex, carried_first: u32, count: u32) -> Self {
        Self { second: u64::from(carried_first) << 32 | u64::from(count), ..Self::on(Opcode::WriteAttribute, at) }
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
    pub(crate) fn cell_in(&self, superchunk: SuperchunkIndex) -> CellIndex {
        CellIndex(superchunk.top_left().0 | u64::from(self.place & ((1 << PLACE_BITS) - 1)))
    }

    /// It, landing in the superchunk of the outbox slot `slot`.
    #[inline]
    pub(crate) fn landing_in(mut self, slot: usize) -> Self {
        self.place = (self.place & ((1 << PLACE_BITS) - 1)) | (slot as u32) << PLACE_BITS;
        self
    }

    /// The outbox slot of the superchunk it lands in.
    #[inline]
    pub(crate) fn outbox_slot(&self) -> usize {
        (self.place >> PLACE_BITS) as usize
    }

    /// It, the blocks it carries beginning `moved` blocks further on
    /// (or back) in the list beside its queue: nothing, of one that
    /// carries none.
    pub(crate) fn carried_moved_by(mut self, moved: i64) -> Self {
        if self.opcode == Opcode::WriteAttribute {
            self.second = (((self.second >> 32) as i64 + moved) as u64) << 32 | (self.second & u64::from(u32::MAX));
        }
        self
    }

    /// The cell of `superchunk` its first word is the place of.
    #[inline]
    pub(crate) fn to(&self, superchunk: SuperchunkIndex) -> CellIndex {
        CellIndex(superchunk.top_left().0 | self.first)
    }

    /// The member of an attribute it is of: its first byte, and how
    /// many bytes it is.
    #[inline]
    pub(crate) fn member(&self) -> (usize, usize) {
        (usize::from(self.small & 0xFF), usize::from(self.small >> 8))
    }

    /// Whether it writes the world -- a plane, an entity, the wheel:
    /// what must lie under a compare.
    #[inline]
    pub(crate) fn is_a_write(&self) -> bool {
        !self.is_a_compare() && !matches!(self.opcode, Opcode::CompareEnd | Opcode::Count)
    }

    /// Whether it names an entity by the cell the tick found it on:
    /// what is to find it wherever it stands as it is applied, in
    /// another superchunk if it crossed.
    #[inline]
    pub(crate) fn names_an_entity(&self) -> bool {
        use Opcode::*;
        matches!(self.opcode, CompareEntity | CompareAttribute | CompareAttributeMember | MoveEntity | CrossEntity | RemoveEntity | WriteWake | WriteAttributeMember | WriteAttribute | RemoveAttribute)
    }

    /// Whether it is a compare: what follows it is under it.
    #[inline]
    pub(crate) fn is_a_compare(&self) -> bool {
        matches!(self.opcode, Opcode::CompareCell | Opcode::CompareCellFree | Opcode::CompareEntity | Opcode::CompareAttribute | Opcode::CompareAttributeMember)
    }
}
