//! Compares: an instruction of their own in what a rule queues. One
//! says what the rule saw -- of a cell, of an entity -- and if that
//! no longer holds when it is come to, everything the same author
//! queued after it is skipped, up to the skip tag closing the
//! innermost skip-tag set open. So the compares of one set let any
//! number of writes be, and a write is a write and nothing else: what
//! it is held against is the compares queued before it
//! (`docs/simulation.md`, "Compare-and-write").

use super::instruction::Instruction;
use super::SimulationTickState;
use crate::chunk_storage::LayerType;
use utilities::coordinates::CellIndex;
use crate::entity_manager::{attribute_seen, AttributeBlock, AttributeType, Member};

/// As many skip-tag sets as are open at once, one within another.
pub(crate) const SKIP_TAG_SETS_WITHIN: usize = 8;
/// As many numbers as a tick counts when applied
/// ([`SimulationTickState::count_under`]): a count's is under this.
pub const COUNTED_WHEN_APPLIED: usize = 256;

/// What a tick counted as it applied: a number each
/// ([`SimulationTickState::count_under`]).
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
        entity_type: crate::entity_manager::EntityType,
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
        /// (`crate::entity_manager::attribute_seen`).
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

// A compare is small whatever it is of.
const _: () = assert!(size_of::<Compare>() <= 40);

/// What a superchunk's rule queues for one superchunk: instructions
/// in the order queued, and the blocks the crossings carry.
#[derive(Default)]
pub(crate) struct InstructionQueue {
    /// The instructions.
    pub(crate) instructions: Vec<Instruction>,
    /// What the entities crossing are made with: for each, a block
    /// whose first two words are its type and its wake, then its
    /// attributes'.
    pub(crate) carried_blocks: Vec<AttributeBlock>,
}

impl InstructionQueue {
    /// Empties it, keeping its room.
    pub(crate) fn clear(&mut self) {
        self.instructions.clear();
        self.carried_blocks.clear();
    }
}

impl SimulationTickState<'_> {
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
    /// under a skip-tag set, whichever superchunk's queue it goes to:
    /// no write is held against nothing
    /// (`docs/simulation.md`, "Compare-and-write", the hard rule).
    #[inline]
    pub(crate) fn queue_instruction(&mut self, landing_cell: CellIndex, instruction: Instruction) -> usize {
        assert!(self.skip_tag_sets_open > 0 || !instruction.is_a_write(), "a write under no compare: {:?}", instruction.opcode);
        let (queue_index, slot) = self.queue_landing_in(landing_cell.superchunk());
        self.note_named(landing_cell, instruction);
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

    /// Queues a skip-tag set and `first_compare` after it: what is queued
    /// until [`SimulationTickState::skip_tag_placed`] is under it, and under every set
    /// open already -- up to [`SKIP_TAG_SETS_WITHIN`] one within another.
    /// The set lands where its first compare does, and so does its
    /// tag: the cell is pushed on the turn's small stack of sets open.
    pub(crate) fn skip_tag_set(&mut self, first_compare: Compare) {
        let first_compared_cell = first_compare.compared_cell();
        assert!(self.skip_tag_sets_open < SKIP_TAG_SETS_WITHIN, "more skip-tag sets one within another than a turn keeps");
        self.skip_tag_set_cells[self.skip_tag_sets_open] = first_compared_cell;
        self.skip_tag_sets_open += 1;
        self.queue_instruction(first_compared_cell, Instruction::set_new_skip_tag());
        self.queue_instruction(first_compared_cell, first_compare.instruction());
    }

    /// Queues `chained_compare` under the innermost skip-tag set open: it
    /// too skips to that set's tag if it does not hold.
    pub(crate) fn compare_chained(&mut self, chained_compare: Compare) {
        assert!(self.skip_tag_sets_open > 0, "a compare under no skip-tag set");
        self.queue_instruction(chained_compare.compared_cell(), chained_compare.instruction());
    }

    /// Queues the compares of the innermost skip-tag set open from here
    /// pointed at the tag of the set `sets_out` sets out of it: 0 its own.
    pub(crate) fn existing_skip_tag_set(&mut self, sets_out: usize) {
        assert!(sets_out < self.skip_tag_sets_open, "a skip tag {sets_out} sets out of {} open", self.skip_tag_sets_open);
        self.queue_instruction(self.skip_tag_set_cells[self.skip_tag_sets_open - 1], Instruction::set_existing_skip_tag(sets_out));
    }

    /// The skip-tag sets open: the depth of the innermost, and one.
    pub(crate) fn skip_tag_sets_open(&self) -> usize {
        self.skip_tag_sets_open
    }

    /// Queues the skip tag closing the innermost skip-tag set open,
    /// popped off the stack: what its compares skip to, and where what
    /// is under it ends. One within another is closed first.
    pub(crate) fn skip_tag_placed(&mut self) {
        self.skip_tag_sets_open -= 1;
        self.queue_instruction(self.skip_tag_set_cells[self.skip_tag_sets_open], Instruction::skip_tag());
    }
}
