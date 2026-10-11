//! A turn's queueing, an instruction at a time: each function here
//! queues one instruction and nothing else -- a compare opened or
//! closed, a write. What is made of several is not the simulation's
//! (`docs/simulation.md`, "Instructions").

use super::instruction::{Instruction, NO_COUNT};
use super::Turn;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeBlock, AttributeType, Header, Member};

impl Turn<'_> {
    /// Queues the compare `compare`, open from here: what is queued
    /// until [`Turn::compare_close`] is under it, and under every
    /// compare open already -- up to
    /// `COMPARES_WITHIN` one within another.
    #[inline]
    pub fn compare_open(&mut self, compare: super::conditional::Compare) {
        self.open_compare(compare);
    }

    /// Queues the end of the compare opened last.
    #[inline]
    pub fn compare_close(&mut self) {
        self.close_compare();
    }

    /// Queues the number of `layer_type` at `at` becoming `value` --
    /// 1 or 0 on a layer of a bit a cell: one added to the rule's
    /// count `counted` as it is applied, if one is given.
    #[inline]
    pub fn write_cell(&mut self, layer_type: LayerType, at: CellIndex, value: u32, counted: Option<u32>) {
        let count = counted.map_or(NO_COUNT, |place| u64::from(self.counted_number(place)));
        self.queue(at, Instruction::write_cell(layer_type, at, value as u16, count));
    }

    /// Queues one added to the rule's count `place`: it lands where
    /// the compare open last does.
    #[inline]
    pub fn count(&mut self, place: u32) {
        let (number, at) = (self.counted_number(place), self.compares_open[self.compares_open_count - 1].at);
        self.queue(at, Instruction::count(number));
    }

    /// Queues an entity made as `header` says: of its type, on its
    /// cell, to wake at its wake.
    #[inline]
    pub fn make_entity(&mut self, header: Header) {
        debug_assert!(header.wake > self.now, "an entity made to wake at tick {}, not after {}", header.wake, self.now);
        self.queue(header.at, Instruction::make_entity(header.at, header.kind.0, header.wake));
    }

    /// Queues the entity the tick found on `from` moved to `to`, in
    /// its superchunk or in another: one instruction either way, the
    /// border a rule's to cross without knowing of it. Over a border
    /// the author lands in both superchunks, so the move is one act
    /// (`docs/simulation.md`, "An act over a border").
    #[inline]
    pub fn move_entity(&mut self, from: CellIndex, to: CellIndex) {
        if from.superchunk() != to.superchunk() {
            self.queue_landing_in(to.superchunk());
        }
        self.queue(from, Self::move_instruction(from, to));
    }

    /// The instruction that moves the entity the tick found on `from`
    /// to `to`: a move within a superchunk, a crossing between two.
    #[inline]
    fn move_instruction(from: CellIndex, to: CellIndex) -> Instruction {
        if from.superchunk() == to.superchunk() {
            Instruction::move_entity(from, to)
        } else {
            Instruction::cross_entity(from, to)
        }
    }

    /// Queues the entity the tick found on `at` removed.
    #[inline]
    pub fn remove_entity(&mut self, at: CellIndex) {
        self.queue(at, Instruction::remove_entity(at));
    }

    /// Queues `member` of the attribute of type `kind` of the entity
    /// the tick found on `at` becoming `value`.
    #[inline]
    pub fn write_attribute_member(&mut self, at: CellIndex, kind: AttributeType, member: Member, value: u64) {
        self.queue(at, Instruction::write_attribute_member(at, kind, member, value));
    }

    /// Queues the entity the tick found on `at` -- or made on it
    /// under the compare open -- given `attribute` whole, the blocks
    /// of one attribute, carried beside the instruction.
    #[inline]
    pub fn write_attribute(&mut self, at: CellIndex, attribute: &[AttributeBlock]) {
        let first = self.carry(at, attribute);
        self.queue(at, Instruction::write_attribute(at, first, attribute.len() as u32));
    }

    /// Queues the attribute of type `kind` taken from the entity the
    /// tick found on `at`.
    #[inline]
    pub fn remove_attribute(&mut self, at: CellIndex, kind: AttributeType) {
        self.queue(at, Instruction::remove_attribute(at, kind));
    }

    /// Queues the entity the tick found on `at` to wake next at
    /// `wake`.
    #[inline]
    pub fn write_wake(&mut self, at: CellIndex, wake: u64) {
        debug_assert!(wake > self.now, "an entity to wake at tick {wake}, not after {}", self.now);
        self.queue(at, Instruction::write_wake(at, wake));
    }
}

/// A write, said whole, for [`Turn::compare_with_writes`]: what one
/// of the turn's writing functions queues, as a value -- those that
/// carry no blocks.
#[derive(Clone, Copy, Debug)]
pub enum Write {
    /// [`Turn::write_cell`].
    Cell {
        /// The layer.
        layer_type: LayerType,
        /// The cell.
        at: CellIndex,
        /// The number it is to hold.
        value: u32,
        /// The rule's count one is added to as it is applied, if any.
        counted: Option<u32>,
    },
    /// [`Turn::count`].
    Count(u32),
    /// [`Turn::make_entity`].
    MakeEntity(Header),
    /// [`Turn::move_entity`].
    MoveEntity {
        /// The cell the tick found it on.
        from: CellIndex,
        /// The cell it moves to, of the same superchunk.
        to: CellIndex,
    },
    /// [`Turn::remove_entity`].
    RemoveEntity(CellIndex),
    /// [`Turn::write_attribute_member`].
    AttributeMember {
        /// The cell the tick found the entity on.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
        /// Which of its members.
        member: Member,
        /// What it is to hold.
        value: u64,
    },
    /// [`Turn::remove_attribute`].
    RemoveAttribute {
        /// The cell the tick found the entity on.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
    },
    /// [`Turn::write_wake`].
    Wake {
        /// The cell the tick found the entity on.
        at: CellIndex,
        /// The tick it is to wake at.
        wake: u64,
    },
}

impl Turn<'_> {
    /// The instruction `write` is queued as, and the cell it lands
    /// on: none for a count, which lands where its compare does.
    #[inline]
    fn instruction_of(&self, write: Write) -> (Option<CellIndex>, Instruction) {
        match write {
            Write::Cell { layer_type, at, value, counted } => (Some(at), Instruction::write_cell(layer_type, at, value as u16, counted.map_or(NO_COUNT, |place| u64::from(self.counted_number(place))))),
            Write::Count(place) => (None, Instruction::count(self.counted_number(place))),
            Write::MakeEntity(header) => (Some(header.at), Instruction::make_entity(header.at, header.kind.0, header.wake)),
            Write::MoveEntity { from, to } => (Some(from), Self::move_instruction(from, to)),
            Write::RemoveEntity(at) => (Some(at), Instruction::remove_entity(at)),
            Write::AttributeMember { at, kind, member, value } => (Some(at), Instruction::write_attribute_member(at, kind, member, value)),
            Write::RemoveAttribute { at, kind } => (Some(at), Instruction::remove_attribute(at, kind)),
            Write::Wake { at, wake } => (Some(at), Instruction::write_wake(at, wake)),
        }
    }

    /// Queues `compare`, the `writes` under it and its end, at once:
    /// what [`Turn::compare_open`], a writing function each and
    /// [`Turn::compare_close`] come to, the queue found once -- most
    /// of what a rule queues is one compare and a write or two. Every
    /// write lands in the compare's superchunk: one landing in
    /// another is queued an instruction at a time.
    #[inline]
    pub fn compare_with_writes(&mut self, compare: super::conditional::Compare, writes: &[Write]) {
        let at = compare.at();
        // A move over a border lands its author in two superchunks: queued an instruction at a time, as what goes to the queue of acts is.
        if writes.iter().any(|write| matches!(write, Write::MoveEntity { from, to } if from.superchunk() != to.superchunk())) {
            self.compare_open(compare);
            for &write in writes {
                match write {
                    Write::MoveEntity { from, to } => self.move_entity(from, to),
                    _ => {
                        let (lands, instruction) = self.instruction_of(write);
                        self.queue(lands.unwrap_or(at), instruction);
                    }
                }
            }
            self.compare_close();
            return;
        }
        let (queue, slot) = self.queue_landing_in(at.superchunk());
        self.note_named(at, compare.instruction());
        let (mut cell_writes, counted_before) = (0, self.outbox.queues[queue].instructions.len());
        self.outbox.queues[queue].instructions.push(compare.instruction().landing_in(slot));
        for &write in writes {
            let (lands, instruction) = self.instruction_of(write);
            debug_assert!(lands.is_none_or(|lands| lands.superchunk() == at.superchunk()), "a write of a compare's group landing in another superchunk");
            cell_writes += u32::from(matches!(write, Write::Cell { .. }));
            if let Some(lands) = lands {
                self.note_named(lands, instruction);
            }
            self.outbox.queues[queue].instructions.push(instruction.landing_in(slot));
        }
        self.writes_queued += cell_writes;
        let queue = &mut self.outbox.queues[queue];
        queue.gated.push(super::conditional::Gated { instructions: (queue.instructions.len() - counted_before - 1) as u32, compares_within: 0, writes: cell_writes as u16 });
        queue.instructions.push(Instruction::compare_end().landing_in(slot));
    }
}
