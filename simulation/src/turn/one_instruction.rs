//! A turn's writes, each one instruction and nothing else: said
//! whole as a [`Write`], and queued one at a time or a compare's
//! worth at once. What is made of several is not the simulation's
//! (`docs/simulation.md`, "Instructions").

use super::conditional::{Compare, Gated};
use super::instruction::{Instruction, NO_COUNT};
use super::Turn;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeBlock, AttributeType, Header, Member};

/// A write, said whole: one instruction, queued by [`Turn::write`]
/// or with its compare by [`Turn::compare_with_writes`] -- every
/// write that carries no blocks ([`Turn::write_attribute`] is the one
/// that does).
#[derive(Clone, Copy, Debug)]
pub enum Write {
    /// The number of `layer_type` at `at` becomes `value` -- 1 or 0
    /// on a layer of a bit a cell.
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
        from: CellIndex,
        /// The cell it moves to.
        to: CellIndex,
    },
    /// The entity the tick found on the cell removed.
    RemoveEntity(CellIndex),
    /// A member of an attribute of the entity the tick found on `at`
    /// becomes `value`.
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
    /// The attribute of type `kind` taken from the entity the tick
    /// found on `at`.
    RemoveAttribute {
        /// The cell the tick found the entity on.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
    },
    /// The entity the tick found on `at` wakes next at `wake`.
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
            Write::MakeEntity(header) => {
                debug_assert!(header.wake > self.now, "an entity made to wake at tick {}, not after {}", header.wake, self.now);
                (Some(header.at), Instruction::make_entity(header.at, header.kind.0, header.wake))
            }
            Write::MoveEntity { from, to } if from.superchunk() == to.superchunk() => (Some(from), Instruction::move_entity(from, to)),
            Write::MoveEntity { from, to } => (Some(from), Instruction::cross_entity(from, to)),
            Write::RemoveEntity(at) => (Some(at), Instruction::remove_entity(at)),
            Write::AttributeMember { at, kind, member, value } => (Some(at), Instruction::write_attribute_member(at, kind, member, value)),
            Write::RemoveAttribute { at, kind } => (Some(at), Instruction::remove_attribute(at, kind)),
            Write::Wake { at, wake } => {
                debug_assert!(wake > self.now, "an entity to wake at tick {wake}, not after {}", self.now);
                (Some(at), Instruction::write_wake(at, wake))
            }
        }
    }

    /// Queues `write`, under every compare open.
    #[inline]
    pub fn write(&mut self, write: Write) {
        // A move over a border lands its author in both superchunks: the one it comes to is named first.
        match write {
            Write::MoveEntity { from, to } if from.superchunk() != to.superchunk() => _ = self.queue_landing_in(to.superchunk()),
            _ => {}
        }
        let (lands, instruction) = self.instruction_of(write);
        self.queue(lands.unwrap_or_else(|| self.compares_open[self.compares_open_count - 1].at), instruction);
    }

    /// Queues the entity the tick found on `at` -- or made on it
    /// under the compare open -- given `attribute` whole, the blocks
    /// of one attribute, carried beside the instruction.
    #[inline]
    pub fn write_attribute(&mut self, at: CellIndex, attribute: &[AttributeBlock]) {
        let first = self.carry(at, attribute);
        self.queue(at, Instruction::write_attribute(at, first, attribute.len() as u32));
    }

    /// Queues `compare`, the `writes` under it and its end, at once:
    /// what [`Turn::compare_open`], [`Turn::write`] of each and
    /// [`Turn::compare_close`] come to, the queue found once -- most
    /// of what a rule queues is one compare and a write or two.
    #[inline]
    pub fn compare_with_writes(&mut self, compare: Compare, writes: &[Write]) {
        let at = compare.at();
        // A move over a border lands its author in two superchunks: queued an instruction at a time, as what goes to the queue of acts is.
        if writes.iter().any(|write| matches!(write, Write::MoveEntity { from, to } if from.superchunk() != to.superchunk())) {
            self.compare_open(compare);
            writes.iter().for_each(|&write| self.write(write));
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
        queue.gated.push(Gated { instructions: (queue.instructions.len() - counted_before - 1) as u32, compares_within: 0, writes: cell_writes as u16 });
        queue.instructions.push(Instruction::compare_end().landing_in(slot));
    }
}
