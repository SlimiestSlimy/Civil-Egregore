//! A turn's writes, each one instruction and nothing else: said
//! whole as a [`Write`], and queued one at a time or a compare's
//! worth at once. What is made of several is not the simulation's
//! (`docs/simulation.md`, "Instructions").

use super::conditional::Compare;
use super::instruction::{Instruction, NO_COUNT};
use super::SimulationTickState;
use crate::chunk_storage::LayerType;
use utilities::coordinates::CellIndex;
use crate::entity_manager::{AttributeBlock, AttributeType, Header, Member};

/// A write, said whole: one instruction, queued by [`SimulationTickState::write`]
/// or with its compare by [`SimulationTickState::compare_with_writes`] -- every
/// write that carries no blocks ([`SimulationTickState::write_attribute`] is the one
/// that does).
#[derive(Clone, Copy, Debug)]
pub enum Write {
    /// The number of `layer_type` at `at` becomes `value` -- 1 or 0
    /// on a layer of a bit a cell.
    Cell {
        /// The layer.
        layer_type: LayerType,
        /// The cell.
        target_cell: CellIndex,
        /// The number it is to hold.
        value: u32,
        /// The rule's count one is added to as it is applied, if any.
        counted_place: Option<u32>,
    },
    /// One added to the rule's count of this place: it lands where
    /// the compare open last does.
    Count(u32),
    /// An entity made as the header says: of its type, on its cell,
    /// to wake at its wake.
    MakeEntity(Header),
    /// The entity the tick found on `from` moved to `to`, in its
    /// superchunk or in another: one instruction either way, the
    /// border a rule's to cross without knowing of it. Over a border
    /// the author lands in both superchunks, so the move is one act
    /// (`docs/simulation.md`, "An act over a border").
    MoveEntity {
        /// The cell the tick found it on.
        from_cell: CellIndex,
        /// The cell it moves to.
        to_cell: CellIndex,
    },
    /// The entity the tick found on the cell removed.
    RemoveEntity(CellIndex),
    /// A member of an attribute of the entity the tick found on `at`
    /// becomes `value`.
    AttributeMember {
        /// The cell the tick found the entity on.
        named_cell: CellIndex,
        /// The attribute's type.
        attribute_type: AttributeType,
        /// Which of its members.
        member: Member,
        /// What it is to hold.
        value: u64,
    },
    /// The attribute of type `kind` taken from the entity the tick
    /// found on `at`.
    RemoveAttribute {
        /// The cell the tick found the entity on.
        named_cell: CellIndex,
        /// The attribute's type.
        attribute_type: AttributeType,
    },
    /// The entity the tick found on `at` wakes next at `wake`.
    Wake {
        /// The cell the tick found the entity on.
        named_cell: CellIndex,
        /// The tick it is to wake at.
        wake: u64,
    },
}

impl SimulationTickState<'_> {
    /// The instruction `write` is queued as, and the cell it lands
    /// on: none for a count, which lands where its compare does.
    #[inline]
    fn instruction_of(&self, write: Write) -> (Option<CellIndex>, Instruction) {
        match write {
            Write::Cell { layer_type, target_cell: at, value, counted_place: counted } => (Some(at), Instruction::write_cell(layer_type, at, value, counted.map_or(NO_COUNT, |place| self.counted_number(place)))),
            Write::Count(place) => (None, Instruction::count_under(self.counted_number(place))),
            Write::MakeEntity(header) => {
                debug_assert!(header.wake > self.current_tick, "an entity made to wake at tick {}, not after {}", header.wake, self.current_tick);
                (Some(header.stood_cell), Instruction::make_entity(header.stood_cell, header.entity_type.0, header.wake))
            }
            Write::MoveEntity { from_cell: from, to_cell: to } if from.superchunk() == to.superchunk() => (Some(from), Instruction::move_entity(from, to)),
            Write::MoveEntity { from_cell: from, to_cell: to } => (Some(from), Instruction::cross_entity(from, to)),
            Write::RemoveEntity(removed_cell) => (Some(removed_cell), Instruction::remove_entity(removed_cell)),
            Write::AttributeMember { named_cell: at, attribute_type: kind, member, value } => (Some(at), Instruction::write_attribute_member(at, kind, member, value)),
            Write::RemoveAttribute { named_cell: at, attribute_type: kind } => (Some(at), Instruction::remove_attribute(at, kind)),
            Write::Wake { named_cell: at, wake } => {
                debug_assert!(wake > self.current_tick, "an entity to wake at tick {wake}, not after {}", self.current_tick);
                (Some(at), Instruction::write_wake(at, wake))
            }
        }
    }

    /// Queues `write`, under every compare open.
    #[inline]
    pub(crate) fn write(&mut self, write: Write) {
        // A move over a border lands its author in both superchunks: the one it comes to is named first.
        match write {
            Write::MoveEntity { from_cell: from, to_cell: to } if from.superchunk() != to.superchunk() => _ = self.queue_landing_in(to.superchunk()),
            _ => {}
        }
        let (landing_cell, instruction) = self.instruction_of(write);
        self.queue_instruction(landing_cell.unwrap_or_else(|| self.skip_tag_set_cells[self.skip_tag_sets_open - 1]), instruction);
    }

    /// Queues the entity the tick found on `named_cell` -- or made on it
    /// under the compare open -- given `attribute` whole, the blocks
    /// of one attribute, carried beside the instruction.
    #[inline]
    pub(crate) fn write_attribute(&mut self, named_cell: CellIndex, attribute: &[AttributeBlock]) {
        let carried_first = self.carry_blocks(named_cell, attribute);
        self.queue_instruction(named_cell, Instruction::write_attribute(named_cell, carried_first, attribute.len() as u32));
    }

    /// Queues a skip-tag set, `group_compare`, the `writes` under it and the
    /// tag, at once: what [`SimulationTickState::skip_tag_set`], [`SimulationTickState::write`] of
    /// each and [`SimulationTickState::skip_tag_placed`] come to, the queue found once
    /// -- most of what a rule queues is one compare and a write or two.
    #[inline]
    pub(crate) fn compare_with_writes(&mut self, group_compare: Compare, writes: &[Write]) {
        let group_cell = group_compare.compared_cell();
        // A move over a border lands its author in two superchunks: queued an instruction at a time, as what goes to the queue of acts is.
        if writes.iter().any(|write| matches!(write, Write::MoveEntity { from_cell, to_cell } if from_cell.superchunk() != to_cell.superchunk())) {
            self.skip_tag_set(group_compare);
            writes.iter().for_each(|&write| self.write(write));
            self.skip_tag_placed();
            return;
        }
        let (queue_index, slot) = self.queue_landing_in(group_cell.superchunk());
        self.note_named(group_cell, group_compare.instruction());
        let group_queue = &mut self.outbox.instruction_queues[queue_index].instructions;
        group_queue.push(Instruction::set_new_skip_tag().landing_in(slot));
        group_queue.push(group_compare.instruction().landing_in(slot));
        for &write in writes {
            let (write_landing, instruction) = self.instruction_of(write);
            debug_assert!(write_landing.is_none_or(|checked_landing| checked_landing.superchunk() == group_cell.superchunk()), "a write of a compare's group landing in another superchunk");
            if let Some(landed_cell) = write_landing {
                self.note_named(landed_cell, instruction);
            }
            self.outbox.instruction_queues[queue_index].instructions.push(instruction.landing_in(slot));
        }
        self.outbox.instruction_queues[queue_index].instructions.push(Instruction::skip_tag().landing_in(slot));
    }
}
