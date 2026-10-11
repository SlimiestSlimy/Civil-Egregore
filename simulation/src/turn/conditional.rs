//! Compares: an instruction of their own in what a rule queues. One
//! says what the rule saw -- of a cell, of an entity -- and if that
//! no longer holds when it is come to, everything the same author
//! queued after it for the superchunk is skipped, up to its next
//! compare or the end of the compare. So one compare lets any number
//! of writes be, and a write is a write and nothing else: what it is
//! held against is a compare queued before it
//! (`docs/simulation.md`, "Compare-and-write").

use super::instruction::{Instruction, NO_COUNT};
use super::Turn;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{attribute_seen, AttributeBlock, AttributeType};

/// As many compares as are open at once, one within another: the
/// rule's, one a write is queued under, the write's own.
pub(crate) const COMPARES_WITHIN: usize = 4;

/// A compare open, on the turn's stack of them.
#[derive(Clone, Copy)]
pub(crate) struct CompareOpen {
    /// The cell it is of.
    at: CellIndex,
    /// Which instruction of its superchunk's queue it is.
    instruction: u32,
    /// Which of the queue's [`Gated`] is its.
    gated: u32,
    /// Writes of cells the turn had queued when it was opened.
    writes_before: u32,
}

impl CompareOpen {
    /// None: what a stack with nothing open holds.
    pub(crate) const NONE: Self = Self { at: CellIndex(0), instruction: 0, gated: 0, writes_before: 0 };
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
        at: CellIndex,
        /// What the rule saw there.
        seen: u16,
    },
    /// The entity of type `kind` the tick found on `at` is there
    /// still, wherever it stands now.
    Entity {
        /// The cell the tick found it on: its name.
        at: CellIndex,
        /// Its type.
        kind: entity_manager::EntityType,
    },
    /// An entity may be made on `at`: nobody stands on it, it is
    /// nobody's name this tick, the collision plane does not hold it.
    CellFree {
        /// The cell.
        at: CellIndex,
    },
    /// The entity the tick found on `at` is there still, wherever it
    /// stands now, and its attribute of type `kind` is as seen -- or
    /// it has none, if none was seen ([`Compare::attribute`]).
    Attribute {
        /// The cell the tick found it on: its name.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
        /// The attribute as the rule saw it, in 64 bits
        /// (`entity_manager::attribute_seen`).
        seen: u64,
    },
    /// The entity the tick found on `at` has the attribute of type
    /// `kind`, and its words from `offset` -- counted after its
    /// first, its type -- are `words`, the first `count` of them.
    AttributeWords {
        /// The cell the tick found it on: its name.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
        /// The first word's offset.
        offset: u16,
        /// How many words: one or two.
        count: u16,
        /// The words as the rule saw them.
        words: [u64; 2],
    },
}

impl Compare {
    /// The entity the tick found on `at`, its attribute of type
    /// `kind` the blocks `seen`, or none.
    pub fn attribute(at: CellIndex, kind: AttributeType, seen: Option<&[AttributeBlock]>) -> Self {
        Self::Attribute { at, kind, seen: attribute_seen(seen) }
    }

    /// The entity the tick found on `at`, the words of its attribute
    /// of type `kind` from `offset` the one or two `words`.
    pub fn attribute_words(at: CellIndex, kind: AttributeType, offset: usize, words: &[u64]) -> Self {
        assert!((1..=2).contains(&words.len()), "{} words in one compare: it has room for two", words.len());
        Self::AttributeWords { at, kind, offset: offset as u16, count: words.len() as u16, words: [words[0], words.get(1).copied().unwrap_or(0)] }
    }

    /// The cell what is compared is on.
    pub(crate) fn at(&self) -> CellIndex {
        match *self {
            Self::Cell { at, .. } | Self::CellFree { at } | Self::Entity { at, .. } | Self::Attribute { at, .. } | Self::AttributeWords { at, .. } => at,
        }
    }

    /// The instruction it is queued as.
    fn instruction(&self) -> Instruction {
        match *self {
            Self::Cell { layer_type, at, seen } => Instruction::compare_cell(layer_type, at, seen),
            Self::CellFree { at } => Instruction::compare_cell_free(at),
            Self::Entity { at, kind } => Instruction::compare_entity(at, kind.0),
            Self::Attribute { at, kind, seen } => Instruction::compare_attribute(at, kind, seen),
            Self::AttributeWords { at, kind, offset, count, words } => Instruction::compare_attribute_words(at, kind, offset as usize, &words[..count as usize]),
        }
    }
}

/// What is under one compare, kept beside the instructions and not in
/// them -- an instruction's room is for what it compares and writes:
/// one of these a compare, in the order the compares are queued, so
/// read in the order they are come to.
#[derive(Clone, Copy, Default)]
pub(crate) struct Gated {
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
const _: () = assert!(size_of::<Compare>() <= 40 && size_of::<Gated>() == 8);

/// What a superchunk's rule queues for one superchunk: instructions
/// in the order queued, what is under each compare among them, and
/// the blocks the crossings carry.
#[derive(Default)]
pub(crate) struct Queue {
    /// The instructions.
    pub(crate) instructions: Vec<Instruction>,
    /// What is under each compare among them, a compare each, in
    /// the order queued.
    pub(crate) gated: Vec<Gated>,
    /// What the entities crossing are made with: for each, a block
    /// whose first two words are its type and its wake, then its
    /// attributes'.
    pub(crate) carried: Vec<AttributeBlock>,
}

impl Queue {
    /// Empties it, keeping its room.
    pub(crate) fn clear(&mut self) {
        self.instructions.clear();
        self.gated.clear();
        self.carried.clear();
    }

    /// How many compares are queued.
    pub(crate) fn compares(&self) -> usize {
        self.gated.len()
    }

    /// How many instructions are queued.
    pub(crate) fn len(&self) -> usize {
        self.instructions.len()
    }
}

impl Turn<'_> {
    /// Has what the rule counts when applied counted from `first` on:
    /// whoever runs several rules on a turn gives each numbers of its
    /// own.
    pub fn count_under(&mut self, first: u32) {
        self.counted_from = first;
    }

    /// The number the rule's count `place` is counted under.
    fn counted_number(&self, place: u32) -> u32 {
        let number = self.counted_from + place;
        assert!((number as usize) < COUNTED_WHEN_APPLIED, "count {place} from {}: more than a tick counts when applied", self.counted_from);
        number
    }

    /// Queues `instruction` for the superchunk of the cell `lands`,
    /// after whatever its author queued before. One that writes lies
    /// under a compare, whichever superchunk's queue it goes to: no
    /// write is held against nothing
    /// (`docs/simulation.md`, "Compare-and-write", the hard rule).
    pub(crate) fn queue(&mut self, lands: CellIndex, instruction: Instruction) {
        assert!(self.compares_open_count > 0 || !instruction.is_a_write(), "a write under no compare: {:?}", instruction.opcode);
        debug_assert!(self.compares_open[..self.compares_open_count].iter().all(|open| open.at.superchunk() == lands.superchunk()), "an instruction under a compare of another superchunk");
        let slot = self.slot_queued(lands.superchunk());
        self.writes_queued += u32::from(instruction.opcode == super::instruction::Opcode::WriteCell);
        self.outbox.queues[slot].instructions.push(instruction);
    }

    /// Queues the compare `compare`, of the cell `at`, the `writes`
    /// under it and its end, at once: what [`Turn::open`], a
    /// [`Turn::queue`] each and [`Turn::close_compare`] come to, the
    /// queue found once -- most of what a rule queues is one compare
    /// and a write or two.
    #[inline]
    pub(crate) fn queue_under(&mut self, at: CellIndex, compare: Instruction, writes: &[Instruction]) {
        debug_assert!(compare.is_a_compare() && writes.iter().all(Instruction::is_a_write));
        debug_assert!(self.compares_open[..self.compares_open_count].iter().all(|open| open.at.superchunk() == at.superchunk()), "an instruction under a compare of another superchunk");
        let slot = self.slot_queued(at.superchunk());
        let cell_writes = writes.iter().filter(|write| write.opcode == super::instruction::Opcode::WriteCell).count();
        self.writes_queued += cell_writes as u32;
        let queue = &mut self.outbox.queues[slot];
        queue.gated.push(Gated { instructions: writes.len() as u32, compares_within: 0, writes: cell_writes as u16 });
        queue.instructions.push(compare);
        queue.instructions.extend_from_slice(writes);
        queue.instructions.push(Instruction::compare_end());
    }

    /// Queues `compare`, open from here ([`Turn::open`]).
    pub(crate) fn open_compare(&mut self, compare: Compare) {
        self.open(compare.at(), compare.instruction());
    }

    /// Queues the compare `instruction`, of the cell `at`, open from
    /// here: what is queued until it is closed is under it, and under
    /// every compare open already. Where it lies in its queue is
    /// pushed on the turn's small stack of compares open.
    pub(crate) fn open(&mut self, at: CellIndex, instruction: Instruction) {
        debug_assert!(instruction.is_a_compare());
        self.queue(at, instruction);
        let queue = &mut self.outbox.queues[self.slot_of(at.superchunk())];
        // Its place among what is under each compare is kept now, in the order the compares are come to, and filled as it is closed.
        let open = CompareOpen { at, instruction: queue.instructions.len() as u32 - 1, gated: queue.gated.len() as u32, writes_before: self.writes_queued };
        queue.gated.push(Gated::default());
        assert!(self.compares_open_count < COMPARES_WITHIN, "more compares one within another than a turn keeps");
        self.compares_open[self.compares_open_count] = open;
        self.compares_open_count += 1;
    }

    /// Closes the compare opened last, popped off the stack: all its
    /// queue holds after it is what is under it -- the queue's end
    /// less where the compare lies, and the compares queued since,
    /// those within it -- noted beside the queue ([`Gated`]), for a
    /// compare that does not hold to jump over; then its end is
    /// queued. One within another is closed first.
    pub(crate) fn close_compare(&mut self) {
        self.compares_open_count -= 1;
        let open = self.compares_open[self.compares_open_count];
        let queue = &mut self.outbox.queues[self.slot_of(open.at.superchunk())];
        let compares_within = u16::try_from(queue.gated.len() - open.gated as usize - 1).expect("more compares within one than is noted");
        let writes = u16::try_from(self.writes_queued - open.writes_before).expect("more writes under one compare than is noted");
        queue.gated[open.gated as usize] = Gated { instructions: queue.instructions.len() as u32 - open.instruction - 1, compares_within, writes };
        self.queue(open.at, Instruction::compare_end());
    }

    /// From here on, what the rule queues for the compare's
    /// superchunk -- cell writes, entity instructions, counts -- is
    /// applied only if `compare` holds when it is come to: skipped
    /// otherwise, up to the next compare given or
    /// [`Turn::compare_end`]. One given takes the place of the one
    /// before. Nothing may be queued for another superchunk
    /// meanwhile: a superchunk applying reads no other, so it could
    /// not be held against the compare.
    pub fn compare_from_here(&mut self, compare: Compare) {
        self.compare_end();
        self.comparing = Some(compare);
        self.open_compare(compare);
    }

    /// From here on, what the rule queues is under no compare.
    pub fn compare_end(&mut self) {
        if self.comparing.take().is_some() {
            self.close_compare();
        }
    }

    /// Queues the number of `layer_type` at `at` becoming `value` if
    /// it is still the `seen` the rule read there, and `compare` --
    /// of anything else in its superchunk -- holds too, when they are
    /// come to: two compares, one within the other, then the write.
    /// Applied, it adds one to the count `counted`, if one is given.
    pub fn queue_if(&mut self, compare: Compare, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        assert_eq!(compare.at().superchunk(), at.superchunk(), "a compare in another superchunk than what it lets be written");
        self.open_compare(compare);
        self.queue_seen(layer_type, at, seen, value, counted);
        self.close_compare();
    }

    /// Queues the number of `layer_type` at `at` becoming `value` if
    /// it is still the `seen` the rule read there: a compare of the
    /// cell, then the write -- so none lands on another's -- under
    /// whatever compare the rule is queuing under as well.
    pub fn queue_seen(&mut self, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        debug_assert!(seen != value, "a write that changes nothing");
        let count = counted.map_or(NO_COUNT, |place| u64::from(self.counted_number(place)));
        self.queue_under(at, Instruction::compare_cell(layer_type, at, seen as u16), &[Instruction::write_cell(layer_type, at, value as u16, count)]);
    }

    /// Adds one to the rule's count `place` if `compare` holds when it
    /// is come to, and whatever compare the rule is queuing under.
    pub fn count_if(&mut self, compare: Compare, place: u32) {
        let number = self.counted_number(place);
        self.open_compare(compare);
        self.queue(compare.at(), Instruction::count(number));
        self.close_compare();
    }
}
