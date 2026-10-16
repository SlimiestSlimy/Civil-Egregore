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
    pub(crate) member_operand: u16,
    /// The cell's place in the superchunk it lands in, and above it
    /// the outbox slot of that superchunk.
    place: u32,
    /// The first word.
    pub(crate) first_operand: u64,
    /// The second.
    pub(crate) second_operand: u64,
}

// The size the instruction set is built to: no opcode has more.
const _: () = assert!(size_of::<Instruction>() == 24 && std::mem::offset_of!(Instruction, opcode) == 0 && coordinates::SUPERCHUNK_SIDE_CELLS as u64 * coordinates::SUPERCHUNK_SIDE_CELLS as u64 == 1 << PLACE_BITS);

impl Instruction {
    /// One of `opcode` on the cell `instruction_cell`, all else 0.
    fn on_cell(opcode: Opcode, instruction_cell: CellIndex) -> Self {
        Self { opcode, member_operand: 0, place: instruction_cell.place_in_superchunk() as u32, first_operand: 0, second_operand: 0 }
    }

    /// The end of a compare.
    pub(crate) fn compare_end() -> Self {
        Self { opcode: Opcode::CompareEnd, member_operand: 0, place: 0, first_operand: 0, second_operand: 0 }
    }

    /// One counted under `counted_number`.
    pub(crate) fn count_under(counted_number: u32) -> Self {
        Self { opcode: Opcode::Count, first_operand: u64::from(counted_number), ..Self::compare_end() }
    }

    /// Whether `layer_type` holds `seen_value` at `target_cell`.
    pub(crate) fn compare_cell(layer_type: LayerType, target_cell: CellIndex, seen_value: u32) -> Self {
        Self { first_operand: layer_type.0, second_operand: u64::from(seen_value), ..Self::on_cell(Opcode::CompareCell, target_cell) }
    }

    /// `layer_type` made to hold `value` at `target_cell`, counted under
    /// `count`, or under nothing ([`NO_COUNT`]): the value the second
    /// word's high half, the count its low.
    pub(crate) fn write_cell(layer_type: LayerType, target_cell: CellIndex, value: u32, count: u32) -> Self {
        Self { first_operand: layer_type.0, second_operand: u64::from(value) << 32 | u64::from(count), ..Self::on_cell(Opcode::WriteCell, target_cell) }
    }

    /// The number a write of a cell makes its cell hold.
    pub(crate) fn value_written(&self) -> u32 {
        (self.second_operand >> 32) as u32
    }

    /// What a write of a cell counts under: [`NO_COUNT`] for nothing.
    pub(crate) fn counted_under(&self) -> u32 {
        self.second_operand as u32
    }

    /// Whether an entity may be made on `target_cell`.
    pub(crate) fn compare_cell_free(target_cell: CellIndex) -> Self {
        Self::on_cell(Opcode::CompareCellFree, target_cell)
    }

    /// Whether the entity of type `entity_type` the tick found on `named_cell` is
    /// there still.
    pub(crate) fn compare_entity(named_cell: CellIndex, entity_type: u64) -> Self {
        Self { first_operand: entity_type, ..Self::on_cell(Opcode::CompareEntity, named_cell) }
    }

    /// An entity of type `entity_type` made on `target_cell`, to wake at `wake`.
    pub(crate) fn make_entity(target_cell: CellIndex, entity_type: u64, wake: u64) -> Self {
        Self { first_operand: entity_type, second_operand: wake, ..Self::on_cell(Opcode::MakeEntity, target_cell) }
    }

    /// The entity on `from_cell` moved to `to_cell`, a cell of its superchunk.
    pub(crate) fn move_entity(from_cell: CellIndex, to_cell: CellIndex) -> Self {
        debug_assert_eq!(from_cell.superchunk(), to_cell.superchunk(), "an entity moved to another superchunk: a crossing");
        Self { first_operand: to_cell.place_in_superchunk() as u64, ..Self::on_cell(Opcode::MoveEntity, from_cell) }
    }

    /// The entity on `named_cell` removed.
    pub(crate) fn remove_entity(named_cell: CellIndex) -> Self {
        Self::on_cell(Opcode::RemoveEntity, named_cell)
    }

    /// The entity the tick found on `from_cell` moved to `to_cell`, a cell of
    /// another superchunk: a crossing. It lands where it stood; the
    /// cell it comes to, whole, is beside it.
    pub(crate) fn cross_entity(from_cell: CellIndex, to_cell: CellIndex) -> Self {
        debug_assert_ne!(from_cell.superchunk(), to_cell.superchunk(), "a crossing within a superchunk: a move");
        Self { first_operand: to_cell.0, ..Self::on_cell(Opcode::CrossEntity, from_cell) }
    }

    /// Whether the attribute `attribute_type` of the entity on `named_cell` sums to `seen_value`.
    pub(crate) fn compare_attribute(named_cell: CellIndex, attribute_type: AttributeType, seen_value: u64) -> Self {
        Self { first_operand: seen_value, second_operand: attribute_type.0, ..Self::on_cell(Opcode::CompareAttribute, named_cell) }
    }

    /// Whether `member` of the attribute `attribute_type` of the entity on `named_cell`
    /// holds `seen_value`.
    pub(crate) fn compare_attribute_member(named_cell: CellIndex, attribute_type: AttributeType, member: Member, seen_value: u64) -> Self {
        Self { member_operand: u16::from(member.byte_offset) | u16::from(member.byte_count) << 8, first_operand: attribute_type.0, second_operand: seen_value, ..Self::on_cell(Opcode::CompareAttributeMember, named_cell) }
    }

    /// `value` written to `member` of the attribute `attribute_type` of the
    /// entity on `named_cell`.
    pub(crate) fn write_attribute_member(named_cell: CellIndex, attribute_type: AttributeType, member: Member, value: u64) -> Self {
        Self { opcode: Opcode::WriteAttributeMember, ..Self::compare_attribute_member(named_cell, attribute_type, member, value) }
    }

    /// The entity on `named_cell` given the attribute that is the `count`
    /// blocks carried from `carried_first` on.
    pub(crate) fn write_attribute(named_cell: CellIndex, carried_first: u32, count: u32) -> Self {
        Self { second_operand: u64::from(carried_first) << 32 | u64::from(count), ..Self::on_cell(Opcode::WriteAttribute, named_cell) }
    }

    /// The attribute `attribute_type` taken from the entity on `named_cell`.
    pub(crate) fn remove_attribute(named_cell: CellIndex, attribute_type: AttributeType) -> Self {
        Self { first_operand: attribute_type.0, ..Self::on_cell(Opcode::RemoveAttribute, named_cell) }
    }

    /// The entity on `named_cell` to wake next at `wake`.
    pub(crate) fn write_wake(named_cell: CellIndex, wake: u64) -> Self {
        Self { first_operand: wake, ..Self::on_cell(Opcode::WriteWake, named_cell) }
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

    /// It, the blocks it carries beginning `carried_shift` blocks further on
    /// (or back) in the list beside its queue: nothing, of one that
    /// carries none.
    pub(crate) fn carried_moved_by(mut self, carried_shift: i64) -> Self {
        if self.opcode == Opcode::WriteAttribute {
            self.second_operand = (((self.second_operand >> 32) as i64 + carried_shift) as u64) << 32 | (self.second_operand & u64::from(u32::MAX));
        }
        self
    }

    /// The cell of `superchunk` its first word is the place of.
    #[inline]
    pub(crate) fn destination_cell(&self, superchunk: SuperchunkIndex) -> CellIndex {
        CellIndex(superchunk.top_left().0 | self.first_operand)
    }

    /// The member of an attribute it is of: its first byte, and how
    /// many bytes it is.
    #[inline]
    pub(crate) fn member(&self) -> (usize, usize) {
        (usize::from(self.member_operand & 0xFF), usize::from(self.member_operand >> 8))
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
