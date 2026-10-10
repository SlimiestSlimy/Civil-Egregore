//! Compare-and-writes: what a rule queues to be applied only if
//! something is still as it saw it -- a cell written, what an entity
//! comes to, a count -- each with a compare of its own, which may look
//! at another thing than it writes
//! (`docs/simulation.md`, "Compare-and-write").

use super::Turn;
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{attribute_blocks, blocks_sum, AttributeBlock, AttributeType, EntityId, Instructions, InstructionsApplied, SuperchunkEntities};

/// A write that counts nothing.
const NO_COUNT: u32 = u32::MAX;
/// As many numbers as a tick counts when applied
/// ([`Turn::count_under`]): a count's is under this.
pub const COUNTED_WHEN_APPLIED: usize = 256;

/// What a tick counted as it applied: a number each
/// ([`Turn::count_under`]).
pub type CountedWhenApplied = [u64; COUNTED_WHEN_APPLIED];

/// What a compare-and-write is held against as it is applied: what
/// its rule saw, of a cell or of an entity, which need not be what it
/// writes -- but is in the superchunk it lands in.
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
    /// The entity `id` stands on `at`, and its attribute of type
    /// `kind` is as seen -- or it has none, if none was seen
    /// ([`Compare::attribute`]).
    Attribute {
        /// The entity.
        id: EntityId,
        /// The cell it stood on.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
        /// The attribute's first block as the rule saw it, if it had
        /// the attribute.
        seen: Option<AttributeBlock>,
        /// The sum of its blocks after the first: of none, for an
        /// attribute one block long.
        rest: u64,
    },
}

impl Compare {
    /// The entity `id` stands on `at`, its attribute of type `kind`
    /// the blocks `seen`, or none.
    pub fn attribute(id: EntityId, at: CellIndex, kind: AttributeType, seen: Option<&[AttributeBlock]>) -> Self {
        let (first, rest) = seen.and_then(|seen| seen.split_first()).map_or((None, &[][..]), |(first, rest)| (Some(*first), rest));
        Self::Attribute { id, at, kind, seen: first, rest: blocks_sum(rest) }
    }

    /// The cell what is compared is on.
    fn at(&self) -> CellIndex {
        match *self {
            Self::Cell { at, .. } | Self::Attribute { at, .. } => at,
        }
    }

    /// Whether it holds now, in `superchunk` and its `entities`: not
    /// where the cell is not hot, nor if the entity no longer stands
    /// there.
    fn holds(&self, superchunk: &Superchunk, entities: &SuperchunkEntities) -> bool {
        match *self {
            Self::Cell { layer_type, at, seen } => superchunk.value_at(layer_type, at) == Some(u32::from(seen)),
            Self::Attribute { id, at, kind, seen, rest } => entities.get_named(id, at).is_some_and(|entity| {
                let now = attribute_blocks(entity.attributes, kind).and_then(|now| now.split_first());
                now.map(|(first, _)| *first) == seen && blocks_sum(now.map_or(&[][..], |(_, rest)| rest)) == rest
            }),
        }
    }
}

/// What a step does if its compare holds.
#[derive(Clone, Copy)]
enum Does {
    /// Makes the number of a layer at a cell `value`, if it is still
    /// the `seen` the rule read there -- whatever else it is held
    /// against, so no write lands on another's -- and counts `count`
    /// if not [`NO_COUNT`].
    Write {
        /// The layer.
        layer_type: LayerType,
        /// The cell.
        at: CellIndex,
        /// What the rule saw there.
        seen: u16,
        /// What it holds after.
        value: u16,
        /// What is counted.
        count: u32,
    },
    /// Applies the instructions of its queue from `first` to before
    /// `last`.
    Instructions {
        /// The first.
        first: u32,
        /// The one past the last.
        last: u32,
    },
    /// Counts one under a number.
    Count(u32),
}

/// A step queued: what it does, under what compare, and how many of
/// its queue's instructions were queued before it -- applied before
/// it.
#[derive(Clone, Copy)]
pub(crate) struct Step {
    /// What it is held against.
    compare: Compare,
    /// What it is held against as well, if anything: both must hold.
    also: Option<Compare>,
    /// What it does.
    does: Does,
    /// Instructions queued before it.
    before: u32,
}

/// What a superchunk's rule queues for one superchunk under compares,
/// in the order queued.
#[derive(Default)]
pub(crate) struct Conditional {
    /// The steps.
    steps: Vec<Step>,
}

/// What a thread applied in a tick's second phase.
pub(crate) struct Applied {
    /// What applying the writes did.
    pub(crate) writes: WritesApplied,
    /// What applying the instructions did.
    pub(crate) instructions: InstructionsApplied,
    /// Runs of instructions under a compare applied, and refused.
    pub(crate) compared: (u64, u64),
    /// What was counted as it was applied.
    pub(crate) counted: CountedWhenApplied,
}

impl Default for Applied {
    fn default() -> Self {
        Self { writes: Default::default(), instructions: Default::default(), compared: (0, 0), counted: [0; COUNTED_WHEN_APPLIED] }
    }
}

impl Conditional {
    /// Empties it, keeping its room.
    pub(crate) fn clear(&mut self) {
        self.steps.clear();
    }

    /// How many steps are queued.
    pub(crate) fn len(&self) -> usize {
        self.steps.len()
    }

    /// Counts every write under a compare as missed: its superchunk is
    /// not hot.
    pub(crate) fn count_missed(&self, applied: &mut WritesApplied) {
        let writes = self.steps.iter().filter(|step| matches!(step.does, Does::Write { .. })).count();
        (applied.writes, applied.missed) = (applied.writes + writes, applied.missed + writes as u64);
    }

    /// Applies the steps at `steps` and those of `instructions` at
    /// `queued` -- one author's ([`super::Segment`]) -- to
    /// `superchunk` and its `entities`, in the order queued: an
    /// instruction under no compare as ever; a step if its compare
    /// holds as it is come to -- so what one applied changed, the
    /// next is held against.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply(&self, steps: std::ops::Range<usize>, queued: std::ops::Range<usize>, superchunk: &mut Superchunk, entities: &mut SuperchunkEntities, instructions: &Instructions, earliest: u64, applied: &mut Applied) {
        let mut next = queued.start;
        for step in &self.steps[steps] {
            instructions.apply_some(next..step.before as usize, std::slice::from_mut(entities), earliest, &mut applied.instructions);
            next = step.before as usize;
            let holds = step.compare.holds(superchunk, entities) && step.also.is_none_or(|also| also.holds(superchunk, entities));
            match step.does {
                Does::Write { layer_type, at, seen, value, count } => {
                    applied.writes.writes += 1;
                    if holds && superchunk.value_at(layer_type, at) == Some(u32::from(seen)) {
                        superchunk.apply(layer_type, Write::cell(at, WriteOp::Put(value)), &mut applied.writes);
                        if count != NO_COUNT {
                            applied.counted[count as usize] += 1;
                        }
                    } else if superchunk.value_at(layer_type, at).is_none() {
                        applied.writes.missed += 1;
                    } else {
                        applied.writes.refused += 1;
                    }
                }
                Does::Instructions { first, last } => {
                    if holds {
                        instructions.apply_some(first as usize..last as usize, std::slice::from_mut(entities), earliest, &mut applied.instructions);
                        applied.compared.0 += 1;
                    } else {
                        applied.compared.1 += 1;
                    }
                    next = last as usize;
                }
                Does::Count(number) => applied.counted[number as usize] += u64::from(holds),
            }
        }
        instructions.apply_some(next..queued.end, std::slice::from_mut(entities), earliest, &mut applied.instructions);
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

    /// Queues `does` under `compare`, in the superchunk of the cell
    /// `lands`: the compare must be of that superchunk too -- what
    /// applies a superchunk reads no other.
    fn queue_step(&mut self, compare: Compare, lands: CellIndex, does: Does) {
        self.close_instructions_compared();
        assert_eq!(compare.at().superchunk(), lands.superchunk(), "a compare in another superchunk than what it lets be written");
        let slot = self.slot_queued(lands.superchunk());
        let before = self.outbox.instructions[slot].len() as u32;
        self.outbox.conditional[slot].steps.push(Step { compare, also: None, does, before });
    }

    /// Queues the number of `layer_type` at `at` becoming `value` if
    /// it is still the `seen` the rule read there, and `compare` --
    /// of anything else in its superchunk -- holds too, when the write
    /// is come to. Refused otherwise, and nothing happens. Applied, it
    /// adds one to the count `counted`, if one is given. Every write of
    /// a cell says what was seen there: none lands on another's.
    pub fn queue_if(&mut self, compare: Compare, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        debug_assert!(seen != value, "a write that changes nothing");
        let count = counted.map_or(NO_COUNT, |place| self.counted_number(place));
        self.queue_step(compare, at, Does::Write { layer_type, at, seen: seen as u16, value: value as u16, count });
    }

    /// [`Turn::queue_if`], held against nothing but the cell written:
    /// still the `seen` the rule read.
    pub fn queue_seen(&mut self, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        self.queue_if(Compare::Cell { layer_type, at, seen: seen as u16 }, layer_type, at, seen, value, counted);
    }

    /// Queues one entity instruction, by `queue`, for the superchunk
    /// of `lands`, under `compare` -- and under the compare the rule's
    /// instructions are being queued under as well, if there is one:
    /// both must hold.
    pub(crate) fn queue_instruction_if(&mut self, compare: Compare, lands: CellIndex, queue: impl FnOnce(&mut Instructions)) {
        self.close_instructions_compared();
        let also = self.comparing.map(|(also, _)| also);
        for compare in also.iter().chain([&compare]) {
            assert_eq!(compare.at().superchunk(), lands.superchunk(), "a compare in another superchunk than what it lets be written");
        }
        let slot = self.slot_queued(lands.superchunk());
        let first = self.outbox.instructions[slot].len() as u32;
        queue(&mut self.outbox.instructions[slot]);
        let last = self.outbox.instructions[slot].len() as u32;
        self.outbox.conditional[slot].steps.push(Step { compare, also, does: Does::Instructions { first, last }, before: first });
        if let Some((_, from)) = &mut self.comparing {
            from[slot] = last;
        }
    }

    /// Adds one to the rule's count `place` if `compare` holds when it
    /// is come to.
    pub fn count_if(&mut self, compare: Compare, place: u32) {
        let number = self.counted_number(place);
        self.queue_step(compare, compare.at(), Does::Count(number));
    }

    /// From here on, every entity instruction the rule queues is under
    /// `compare` -- applied only if it holds when the instruction is
    /// come to -- until [`Turn::instructions_as_ever`], or another
    /// compare is given. They must land in the compare's superchunk.
    pub fn instructions_if(&mut self, compare: Compare) {
        self.close_instructions_compared();
        let outbox = &*self.outbox;
        self.comparing = Some((compare, std::array::from_fn(|slot| outbox.instructions[slot].len() as u32)));
    }

    /// From here on, the entity instructions the rule queues are under
    /// no compare.
    pub fn instructions_as_ever(&mut self) {
        self.close_instructions_compared();
        self.comparing = None;
    }

    /// Makes the instructions queued under the compare given so far a
    /// step, in the order they came: before whatever is queued next.
    pub(crate) fn close_instructions_compared(&mut self) {
        let Some((compare, from)) = &mut self.comparing else {
            return;
        };
        let lands = super::slot_between(self.superchunk.index(), compare.at().superchunk());
        for (slot, from) in from.iter_mut().enumerate() {
            let last = self.outbox.instructions[slot].len() as u32;
            if last != *from {
                assert_eq!(Some(slot), lands, "an instruction under a compare in another superchunk");
                self.outbox.conditional[slot].steps.push(Step { compare: *compare, also: None, does: Does::Instructions { first: *from, last }, before: *from });
                *from = last;
            }
        }
    }
}
