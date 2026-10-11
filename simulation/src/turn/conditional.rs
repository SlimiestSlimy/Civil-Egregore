//! Compares: an instruction of their own in what a rule queues. One
//! says what the rule saw -- of a cell, of an entity -- and if that
//! no longer holds when it is come to, everything the same author
//! queued after it for the superchunk is skipped, up to its next
//! compare or the end of the compare. So one compare lets any number
//! of writes be, and a write carries no compare but of what it writes
//! (`docs/simulation.md`, "Compare-and-write").

use super::Turn;
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{attribute_blocks, attribute_seen, AttributeBlock, AttributeType, CollisionCells, EntityId, Instructions, InstructionsApplied, SuperchunkEntities};

/// A write that counts nothing.
const NO_COUNT: u32 = u32::MAX;
/// As many numbers as a tick counts when applied
/// ([`Turn::count_under`]): a count's is under this.
pub const COUNTED_WHEN_APPLIED: usize = 256;

/// What a tick counted as it applied: a number each
/// ([`Turn::count_under`]).
pub type CountedWhenApplied = [u64; COUNTED_WHEN_APPLIED];

/// What a rule saw, held against what is there as the compare is
/// come to: of a cell or of an entity, in the superchunk what it
/// lets be written lands in. 64 bits of what was seen at the most:
/// never a block.
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
        /// The attribute as the rule saw it, in 64 bits
        /// (`entity_manager::attribute_seen`).
        seen: u64,
    },
}

impl Compare {
    /// The entity `id` stands on `at`, its attribute of type `kind`
    /// the blocks `seen`, or none.
    pub fn attribute(id: EntityId, at: CellIndex, kind: AttributeType, seen: Option<&[AttributeBlock]>) -> Self {
        Self::Attribute { id, at, kind, seen: attribute_seen(seen) }
    }

    /// The cell what is compared is on.
    pub(crate) fn at(&self) -> CellIndex {
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
            Self::Attribute { id, at, kind, seen } => entities.get_named(id, at).is_some_and(|entity| attribute_seen(attribute_blocks(entity.attributes, kind)) == seen),
        }
    }
}

/// A superchunk's collision plane as its entities keep it while their
/// instructions are applied: `layer` of `superchunk`, or none -- a
/// world with no such plane (`docs/simulation.md`, "Entities").
pub(crate) struct Standing<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a mut Superchunk,
    /// The collision plane's layer, if entities are kept in one.
    pub(crate) layer: Option<LayerType>,
}

impl Standing<'_> {
    /// Writes `to` at `at` of the plane, if there is one. Not counted
    /// among the tick's writes: it is part of the entity's instruction.
    fn write(&mut self, at: CellIndex, to: WriteOp) {
        if let Some(layer) = self.layer {
            self.superchunk.apply(layer, Write::cell(at, to), &mut WritesApplied::default());
        }
    }
}

impl CollisionCells for Standing<'_> {
    fn held(&self, at: CellIndex) -> bool {
        self.layer.is_some_and(|layer| self.superchunk.value_at(layer, at) == Some(1))
    }

    fn hold(&mut self, at: CellIndex) {
        self.write(at, WriteOp::Set);
    }

    fn free(&mut self, at: CellIndex) {
        self.write(at, WriteOp::Unset);
    }
}

/// What a step is.
#[derive(Clone, Copy)]
enum Does {
    /// A compare: if it does not hold as it is come to, what its
    /// author queued after it for the superchunk is skipped, up to
    /// the author's next compare or [`Does::CompareEnd`].
    Compare(Compare),
    /// The end of a compare: what follows is skipped no more.
    CompareEnd,
    /// Makes the number of a layer at a cell `value`, if it is still
    /// the `seen` the rule read there -- a compare of its own, of
    /// what it writes, so no write lands on another's -- and counts
    /// `count` if not [`NO_COUNT`].
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
    /// Counts one under a number.
    Count(u32),
}

/// A step queued: what it is, and how many of its queue's entity
/// instructions were queued before it -- come to before it.
#[derive(Clone, Copy)]
pub(crate) struct Step {
    /// What it is.
    does: Does,
    /// Instructions queued before it.
    before: u32,
    /// Of a compare, the steps after it that are under it -- up to
    /// its author's next compare or the end of the compare -- noted
    /// as they are queued: what a compare that does not hold jumps
    /// over at once, not step by step.
    under: u16,
    /// Of a compare, how many of those are writes of cells: counted
    /// as refused when jumped over.
    writes_under: u16,
}

// A step is small whatever it is: no compare carries a block.
const _: () = assert!(size_of::<Step>() <= 48 && size_of::<Compare>() <= 40);

/// What a superchunk's rule queues for one superchunk beside its
/// entity instructions -- compares, cell writes, counts -- in the
/// order queued.
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
    /// Compares come to that held, and that did not.
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
    /// `superchunk` and its `entities`, in the order queued, each as
    /// it is come to. A compare that does not hold has what follows
    /// skipped, writes, instructions and counts alike, up to the next
    /// compare or the end of the compare: so what one applied
    /// changed, the next compare is held against. `collision` is the
    /// layer entities are kept in, if the world has one ([`Standing`]).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply(&self, steps: std::ops::Range<usize>, queued: std::ops::Range<usize>, superchunk: &mut Superchunk, entities: &mut SuperchunkEntities, instructions: &Instructions, earliest: u64, collision: Option<LayerType>, applied: &mut Applied) {
        let (mut next, mut at_step, last_step) = (queued.start, steps.start, steps.end);
        while at_step < last_step {
            let step = &self.steps[at_step];
            at_step += 1;
            instructions.apply_some(next..step.before as usize, std::slice::from_mut(entities), earliest, &mut applied.instructions, &mut Standing { superchunk, layer: collision });
            next = step.before as usize;
            match step.does {
                Does::Compare(compare) => {
                    if compare.holds(superchunk, entities) {
                        applied.compared.0 += 1;
                        continue;
                    }
                    applied.compared.1 += 1;
                    // Over all that is under it at once: to the author's next compare, the end of this one, or the end of what the author queued.
                    at_step += step.under as usize;
                    (applied.writes.writes, applied.writes.refused) = (applied.writes.writes + step.writes_under as usize, applied.writes.refused + u64::from(step.writes_under));
                    next = self.steps[..last_step].get(at_step).map_or(queued.end, |after| after.before as usize);
                }
                Does::CompareEnd => {}
                Does::Write { layer_type, at, seen, value, count } => {
                    applied.writes.writes += 1;
                    if superchunk.value_at(layer_type, at) == Some(u32::from(seen)) {
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
                Does::Count(number) => applied.counted[number as usize] += 1,
            }
        }
        instructions.apply_some(next..queued.end, std::slice::from_mut(entities), earliest, &mut applied.instructions, &mut Standing { superchunk, layer: collision });
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

    /// Queues `does` for the superchunk of the cell `lands`, after
    /// whatever its author queued before.
    fn queue_step(&mut self, lands: CellIndex, does: Does) {
        let slot = self.slot_queued(lands.superchunk());
        let before = self.outbox.instructions[slot].len() as u32;
        let steps = &mut self.outbox.conditional[slot].steps;
        // Noted on the compare it is under, if any: what a compare that fails jumps over.
        if let (Some((_, compare)), false) = (self.comparing, matches!(does, Does::Compare(_) | Does::CompareEnd)) {
            let compare = &mut steps[compare as usize];
            compare.under = compare.under.checked_add(1).expect("more under one compare than a compare notes");
            compare.writes_under += u16::from(matches!(does, Does::Write { .. }));
        }
        steps.push(Step { does, before, under: 0, writes_under: 0 });
    }

    /// From here on, what the rule queues for the compare's
    /// superchunk -- cell writes, entity instructions, counts -- is
    /// applied only if `compare` holds when it is come to: skipped
    /// otherwise, up to the next compare given or
    /// [`Turn::compare_end`]. Compares do not nest: one given takes
    /// the place of the one before. Nothing may be queued for another
    /// superchunk meanwhile: a superchunk applying reads no other, so
    /// it could not be held against the compare.
    pub fn compare_from_here(&mut self, compare: Compare) {
        self.comparing = None;
        self.queue_step(compare.at(), Does::Compare(compare));
        let slot = self.slot_of(compare.at().superchunk());
        self.comparing = Some((compare, self.outbox.conditional[slot].steps.len() as u32 - 1));
    }

    /// From here on, what the rule queues is under no compare.
    pub fn compare_end(&mut self) {
        if let Some((compare, _)) = self.comparing.take() {
            self.queue_step(compare.at(), Does::CompareEnd);
        }
    }

    /// Queues `does`, landing on `lands`, under `compare` alone, and
    /// takes up again the compare the rule was queuing under, if it
    /// was under another: held anew when come to.
    fn queue_under(&mut self, compare: Compare, lands: CellIndex, does: Does) {
        assert_eq!(compare.at().superchunk(), lands.superchunk(), "a compare in another superchunk than what it lets be written");
        if self.comparing.is_some_and(|(under, _)| under == compare) {
            return self.queue_step(lands, does);
        }
        let under = self.comparing.map(|(under, _)| under);
        self.compare_from_here(compare);
        self.queue_step(lands, does);
        self.compare_end();
        if let Some(under) = under {
            self.compare_from_here(under);
        }
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
        self.queue_under(compare, at, Does::Write { layer_type, at, seen: seen as u16, value: value as u16, count });
    }

    /// Queues the number of `layer_type` at `at` becoming `value` if
    /// it is still the `seen` the rule read there -- and, if the rule
    /// is queuing under a compare of the cell's superchunk, that holds.
    pub fn queue_seen(&mut self, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        debug_assert!(seen != value, "a write that changes nothing");
        let count = counted.map_or(NO_COUNT, |place| self.counted_number(place));
        self.queue_step(at, Does::Write { layer_type, at, seen: seen as u16, value: value as u16, count });
    }

    /// Adds one to the rule's count `place` if `compare` holds when it
    /// is come to.
    pub fn count_if(&mut self, compare: Compare, place: u32) {
        let number = self.counted_number(place);
        self.queue_under(compare, compare.at(), Does::Count(number));
    }
}
