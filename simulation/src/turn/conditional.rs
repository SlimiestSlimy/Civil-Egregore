//! Compares: an instruction of their own in what a rule queues. One
//! says what the rule saw -- of a cell, of an entity -- and if that
//! no longer holds when it is come to, everything the same author
//! queued after it for the superchunk is skipped, up to its next
//! compare or the end of the compare. So one compare lets any number
//! of writes be, and a write is a write and nothing else: what it is
//! held against is a compare queued before it
//! (`docs/simulation.md`, "Compare-and-write").

use super::instruction::Instruction;
use super::Turn;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{attribute_seen, AttributeBlock, AttributeType, Member};

/// As many compares as are open at once, one within another: the
/// rule's, one a write is queued under, the write's own.
pub(crate) const COMPARES_WITHIN: usize = 4;

/// A compare open, on the turn's stack of them.
#[derive(Clone, Copy)]
pub(crate) struct CompareOpen {
    /// The cell it is of.
    pub(crate) compared_cell: CellIndex,
    /// Which instruction of its queue it is.
    pub(crate) instruction: u32,
    /// Which of the queue's [`GatedByCompare`] is its.
    pub(crate) gated_index: u32,
    /// Writes of cells the turn had queued when it was opened.
    pub(crate) writes_before: u32,
    /// Blocks its queue carried when it was opened.
    pub(crate) carried_before: u32,
    /// Which of the outbox's queues it lies in.
    pub(crate) queue_index: u8,
}

impl CompareOpen {
    /// None: what a stack with nothing open holds.
    pub(crate) const NONE_OPEN: Self = Self { compared_cell: CellIndex(0), instruction: 0, gated_index: 0, writes_before: 0, carried_before: 0, queue_index: 0 };
}
/// As many numbers as a tick counts when applied
/// ([`Turn::count_under`]): a count's is under this.
pub const COUNTED_WHEN_APPLIED: usize = 256;

/// What a tick counted as it applied: a number each
/// ([`Turn::count_under`]).
pub type CountedWhenApplied = [u64; COUNTED_WHEN_APPLIED];

/// What a rule saw, held against what is there as the compare is
/// come to: of a cell or of an entity, in the superchunk what it
/// lets be written lands in. Sixteen bytes of what was seen at the
/// most: never a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    /// The number `layer_type` holds at `at` is `seen` -- 1 or 0 on a
    /// layer of a bit a cell.
    Cell {
        /// The layer.
        layer_type: LayerType,
        /// The cell.
        target_cell: CellIndex,
        /// What the rule saw there.
        seen_value: u32,
    },
    /// The entity of type `kind` the tick found on `at` is there
    /// still, wherever it stands now.
    Entity {
        /// The cell the tick found it on: its name.
        named_cell: CellIndex,
        /// Its type.
        entity_type: entity_manager::EntityType,
    },
    /// An entity may be made on `at`: nobody stands on it, it is
    /// nobody's name this tick, the collision plane does not hold it.
    CellFree {
        /// The cell.
        target_cell: CellIndex,
    },
    /// The entity the tick found on `at` is there still, wherever it
    /// stands now, and its attribute of type `kind` is as seen -- or
    /// it has none, if none was seen ([`Compare::attribute`]).
    Attribute {
        /// The cell the tick found it on: its name.
        named_cell: CellIndex,
        /// The attribute's type.
        attribute_type: AttributeType,
        /// The attribute as the rule saw it, in 64 bits
        /// (`entity_manager::attribute_seen`).
        seen_value: u64,
    },
    /// The entity the tick found on `at` has the attribute of type
    /// `kind`, and `member` of it holds `seen`.
    AttributeMember {
        /// The cell the tick found it on: its name.
        named_cell: CellIndex,
        /// The attribute's type.
        attribute_type: AttributeType,
        /// Which of its members.
        member: Member,
        /// What the rule saw it hold.
        seen_value: u64,
    },
}

impl Compare {
    /// The entity the tick found on `named_cell`, its attribute of type
    /// `attribute_type` the blocks `seen_blocks`, or none.
    pub fn attribute(named_cell: CellIndex, attribute_type: AttributeType, seen_blocks: Option<&[AttributeBlock]>) -> Self {
        Self::Attribute { named_cell, attribute_type, seen_value: attribute_seen(seen_blocks) }
    }

    /// The cell what is compared is on.
    pub(crate) fn compared_cell(&self) -> CellIndex {
        match *self {
            Self::Cell { target_cell: at, .. } | Self::CellFree { target_cell: at } | Self::Entity { named_cell: at, .. } | Self::Attribute { named_cell: at, .. } | Self::AttributeMember { named_cell: at, .. } => at,
        }
    }

    /// The instruction it is queued as.
    pub(crate) fn instruction(&self) -> Instruction {
        match *self {
            Self::Cell { layer_type, target_cell: at, seen_value: seen } => Instruction::compare_cell(layer_type, at, seen),
            Self::CellFree { target_cell: at } => Instruction::compare_cell_free(at),
            Self::Entity { named_cell: at, entity_type: kind } => Instruction::compare_entity(at, kind.0),
            Self::Attribute { named_cell: at, attribute_type: kind, seen_value: seen } => Instruction::compare_attribute(at, kind, seen),
            Self::AttributeMember { named_cell: at, attribute_type: kind, member, seen_value: seen } => Instruction::compare_attribute_member(at, kind, member, seen),
        }
    }
}

/// What is under one compare, kept beside the instructions and not in
/// them -- an instruction's room is for what it compares and writes:
/// one of these a compare, in the order the compares are queued, so
/// read in the order they are come to.
#[derive(Clone, Copy, Default)]
pub(crate) struct GatedByCompare {
    /// The instructions after the compare that are under it, up to
    /// its end: what it jumps over if it does not hold.
    pub(crate) instructions: u32,
    /// The compares within it: as many of these are passed over with
    /// it.
    pub(crate) compares_within: u16,
    /// How many of the instructions under it are writes of cells:
    /// counted as refused when jumped over.
    pub(crate) writes: u16,
}

// A compare is small whatever it is of, and what is under one is noted in eight bytes.
const _: () = assert!(size_of::<Compare>() <= 40 && size_of::<GatedByCompare>() == 8);

/// What a superchunk's rule queues for one superchunk: instructions
/// in the order queued, what is under each compare among them, and
/// the blocks the crossings carry.
#[derive(Default)]
pub(crate) struct InstructionQueue {
    /// The instructions.
    pub(crate) instructions: Vec<Instruction>,
    /// What is under each compare among them, a compare each, in
    /// the order queued.
    pub(crate) gated_by_compares: Vec<GatedByCompare>,
    /// What the entities crossing are made with: for each, a block
    /// whose first two words are its type and its wake, then its
    /// attributes'.
    pub(crate) carried_blocks: Vec<AttributeBlock>,
}

impl InstructionQueue {
    /// Empties it, keeping its room.
    pub(crate) fn clear(&mut self) {
        self.instructions.clear();
        self.gated_by_compares.clear();
        self.carried_blocks.clear();
    }
}

impl Turn<'_> {
    /// Has what the rule counts when applied counted from `first_counted_number` on:
    /// whoever runs several rules on a turn gives each numbers of its
    /// own.
    pub fn count_under(&mut self, first_counted_number: u32) {
        self.counted_from = first_counted_number;
    }

    /// The number the rule's count `place` is counted under.
    pub(crate) fn counted_number(&self, place: u32) -> u32 {
        let count_number = self.counted_from + place;
        assert!((count_number as usize) < COUNTED_WHEN_APPLIED, "count {place} from {}: more than a tick counts when applied", self.counted_from);
        count_number
    }

    /// Queues `instruction` for the superchunk of the cell `landing_cell`,
    /// after whatever its author queued before. One that writes lies
    /// under a compare, whichever superchunk's queue it goes to: no
    /// write is held against nothing
    /// (`docs/simulation.md`, "Compare-and-write", the hard rule).
    #[inline]
    pub(crate) fn queue_instruction(&mut self, landing_cell: CellIndex, instruction: Instruction) -> usize {
        assert!(self.compares_open_count > 0 || !instruction.is_a_write(), "a write under no compare: {:?}", instruction.opcode);
        let (queue_index, slot) = self.queue_landing_in(landing_cell.superchunk());
        self.note_named(landing_cell, instruction);
        self.writes_queued += u32::from(instruction.opcode == super::instruction::Opcode::WriteCell);
        self.outbox.instruction_queues[queue_index].instructions.push(instruction.landing_in(slot));
        queue_index
    }

    /// Notes, of `instruction` naming the entity the tick found on
    /// `named_cell`, that its author is not that entity: should the entity
    /// cross a border this tick, what this author queued for it is
    /// to follow it ([`super::Outbox::named_by_others`]). An author
    /// nearly always names itself, and nothing is noted.
    #[inline]
    pub(crate) fn note_named(&mut self, named_cell: CellIndex, instruction: Instruction) {
        if named_cell.0 + 1 != self.author && self.author != 0 && instruction.names_an_entity() {
            self.outbox.named_by_others.push((named_cell, self.author));
        }
    }

    /// Carries `attribute_blocks` beside the queue an instruction landing in
    /// the superchunk of `landing_cell` goes to: where they begin there.
    pub(crate) fn carry_blocks(&mut self, landing_cell: CellIndex, attribute_blocks: &[AttributeBlock]) -> u32 {
        let (queue_index, _) = self.queue_landing_in(landing_cell.superchunk());
        let queue_carried = &mut self.outbox.instruction_queues[queue_index].carried_blocks;
        queue_carried.extend_from_slice(attribute_blocks);
        (queue_carried.len() - attribute_blocks.len()) as u32
    }

    /// Queues `opened_compare`, open from here: what is queued until
    /// [`Turn::compare_close`] is under it, and under every compare
    /// open already -- up to [`COMPARES_WITHIN`] one within another.
    /// Where it lies in its queue is pushed on the turn's small stack
    /// of compares open.
    pub fn compare_open(&mut self, opened_compare: Compare) {
        let opened_cell = opened_compare.compared_cell();
        // Where the instruction went: its slot's queue, or that of acts over a border.
        let queue_index = self.queue_instruction(opened_cell, opened_compare.instruction());
        let opened_queue = &mut self.outbox.instruction_queues[queue_index];
        // Its place among what is under each compare is kept now, in the order the compares are come to, and filled as it is closed.
        let open_compare = CompareOpen { compared_cell: opened_cell, instruction: opened_queue.instructions.len() as u32 - 1, gated_index: opened_queue.gated_by_compares.len() as u32, writes_before: self.writes_queued, carried_before: opened_queue.carried_blocks.len() as u32, queue_index: queue_index as u8 };
        opened_queue.gated_by_compares.push(GatedByCompare::default());
        assert!(self.compares_open_count < COMPARES_WITHIN, "more compares one within another than a turn keeps");
        self.compares_open[self.compares_open_count] = open_compare;
        self.compares_open_count += 1;
    }

    /// Queues the end of the compare opened last, popped off the stack: all its
    /// queue holds after it is what is under it -- the queue's end
    /// less where the compare lies, and the compares queued since,
    /// those within it -- noted beside the queue ([`GatedByCompare`]), for a
    /// compare that does not hold to jump over; then its end is
    /// queued. One within another is closed first.
    pub fn compare_close(&mut self) {
        self.compares_open_count -= 1;
        let closing_compare = self.compares_open[self.compares_open_count];
        let closing_queue = &mut self.outbox.instruction_queues[closing_compare.queue_index as usize];
        let compares_within = u16::try_from(closing_queue.gated_by_compares.len() - closing_compare.gated_index as usize - 1).expect("more compares within one than is noted");
        let writes = u16::try_from(self.writes_queued - closing_compare.writes_before).expect("more writes under one compare than is noted");
        closing_queue.gated_by_compares[closing_compare.gated_index as usize] = GatedByCompare { instructions: closing_queue.instructions.len() as u32 - closing_compare.instruction - 1, compares_within, writes };
        self.queue_instruction(closing_compare.compared_cell, Instruction::compare_end());
    }

    /// From here on, what the rule queues for the compare's
    /// superchunk -- cell writes, entity instructions, counts -- is
    /// applied only if `gating_compare` holds when it is come to: skipped
    /// otherwise, up to the next compare given or
    /// [`Turn::compare_end`]. One given takes the place of the one
    /// before. It gates what is queued for another superchunk
    /// meanwhile too: all an author queues for more than one is
    /// applied as one (`docs/simulation.md`, "An act over a border").
    pub fn compare_from_here(&mut self, gating_compare: Compare) {
        self.compare_end();
        self.is_comparing = true;
        self.compare_open(gating_compare);
    }

    /// From here on, what the rule queues is under no compare.
    pub fn compare_end(&mut self) {
        if std::mem::take(&mut self.is_comparing) {
            self.compare_close();
        }
    }
}
