//! Writes that say what their rule saw, and groups: what a rule
//! queues to be applied only if the world is still as it saw it, one
//! write alone or several things as one
//! (`docs/simulation.md`, "Compare-and-write and groups").

use super::{Turn, SLOTS};
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{Instructions, InstructionsApplied, SuperchunkEntities};

/// A write in no group.
const NO_GROUP: u32 = u32::MAX;
/// A write, or a group, that counts nothing.
const NO_COUNT: u32 = u32::MAX;
/// As many numbers as a tick counts when applied
/// ([`Turn::count_under`]): a count's is under this.
pub const COUNTED_WHEN_APPLIED: usize = 256;

/// What a tick counted as it applied: a number each
/// ([`Turn::count_under`]).
pub type CountedWhenApplied = [u64; COUNTED_WHEN_APPLIED];

/// A compare-and-write: a cell's number made `value` if it is still
/// the `seen` its rule read.
#[derive(Clone, Copy)]
pub(crate) struct Compared {
    /// The layer.
    layer_type: LayerType,
    /// The cell.
    at: CellIndex,
    /// What the rule saw there.
    seen: u16,
    /// What it makes of it.
    value: u16,
    /// Its group among those of its queue, or [`NO_GROUP`].
    group: u32,
    /// What is counted if it is applied, or [`NO_COUNT`]: of a write
    /// in no group.
    count: u32,
}

/// A group queued: instructions and counts, and the compare-and-writes
/// marked as its own, all applied or none.
#[derive(Clone, Copy)]
pub(crate) struct Group {
    /// Its instructions, among its queue's: the first, and the one
    /// past the last.
    instructions: (u32, u32),
    /// Its counts, among its queue's.
    counts: (u32, u32),
}

/// What a superchunk's rule queues for one superchunk to be applied
/// only as the world still is.
#[derive(Default)]
pub(crate) struct Conditional {
    /// The compare-and-writes, in the order queued: a group's together.
    compared: Vec<Compared>,
    /// The groups, in the order queued.
    groups: Vec<Group>,
    /// The groups' counts.
    counts: Vec<u32>,
}

/// A group being queued: how much each slot of the outbox held when it
/// was started.
#[derive(Clone, Copy)]
pub(crate) struct OpenGroup {
    /// Plain writes queued, a slot each.
    writes: [u32; SLOTS],
    /// Compare-and-writes queued, a slot each.
    compared: [u32; SLOTS],
    /// Instructions queued, a slot each.
    instructions: [u32; SLOTS],
}

impl Conditional {
    /// Empties it, keeping its room.
    pub(crate) fn clear(&mut self) {
        self.compared.clear();
        self.groups.clear();
        self.counts.clear();
    }

    /// Counts every compare-and-write as missed: its superchunk is not
    /// hot.
    pub(crate) fn count_missed(&self, applied: &mut WritesApplied) {
        applied.writes += self.compared.len();
        applied.missed += self.compared.len() as u64;
    }

    /// Applies what is queued to `superchunk` and its `entities`, after
    /// the plain writes of the same queue: each compare-and-write in no
    /// group if its cell still holds what was seen; each group, in the
    /// order queued, if every one of its compare-and-writes does --
    /// then its writes, and with them its instructions and counts --
    /// and `instructions`, in the order queued, those of the groups
    /// refused left out. `fates` is room for the groups' fates.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply(&self, superchunk: &mut Superchunk, entities: &mut SuperchunkEntities, instructions: &Instructions, earliest: u64, fates: &mut Vec<bool>, applied: &mut Applied) {
        fates.clear();
        fates.resize(self.groups.len(), true);
        let mut first = 0;
        while first < self.compared.len() {
            let group = self.compared[first].group;
            let together = if group == NO_GROUP { 1 } else { self.compared[first..].iter().take_while(|write| write.group == group).count() };
            let writes = &self.compared[first..first + together];
            first += together;
            applied.writes.writes += together;
            if !writes.iter().all(|write| superchunk.value_at(write.layer_type, write.at) == Some(u32::from(write.seen))) {
                // Not hot there, it is missed; hot, the cell is no longer what was seen.
                let missed = writes.iter().filter(|write| superchunk.value_at(write.layer_type, write.at).is_none()).count() as u64;
                (applied.writes.missed, applied.writes.refused) = (applied.writes.missed + missed, applied.writes.refused + together as u64 - missed);
                if group != NO_GROUP {
                    fates[group as usize] = false;
                }
                continue;
            }
            for write in writes {
                superchunk.apply(write.layer_type, Write::cell(write.at, WriteOp::Put(write.value)), &mut applied.writes);
                if write.count != NO_COUNT {
                    applied.counted[write.count as usize] += 1;
                }
            }
        }
        let mut next = 0;
        for (group, &applies) in self.groups.iter().zip(fates.iter()) {
            if applies {
                applied.groups_applied += 1;
                self.counts[group.counts.0 as usize..group.counts.1 as usize].iter().for_each(|&count| applied.counted[count as usize] += 1);
                continue;
            }
            applied.groups_refused += 1;
            instructions.apply_some(next..group.instructions.0 as usize, std::slice::from_mut(entities), earliest, &mut applied.instructions);
            next = group.instructions.1 as usize;
        }
        instructions.apply_some(next..instructions.len(), std::slice::from_mut(entities), earliest, &mut applied.instructions);
    }
}

/// What a thread applied in a tick's second phase.
pub(crate) struct Applied {
    /// What applying the writes did.
    pub(crate) writes: WritesApplied,
    /// What applying the instructions did.
    pub(crate) instructions: InstructionsApplied,
    /// Groups applied.
    pub(crate) groups_applied: u64,
    /// Groups refused.
    pub(crate) groups_refused: u64,
    /// What was counted as it was applied.
    pub(crate) counted: CountedWhenApplied,
}

impl Default for Applied {
    fn default() -> Self {
        Self { writes: Default::default(), instructions: Default::default(), groups_applied: 0, groups_refused: 0, counted: [0; COUNTED_WHEN_APPLIED] }
    }
}

impl Turn<'_> {
    /// Has what the rule counts when applied ([`Turn::queue_seen`],
    /// [`Turn::count_if_applied`]) counted from `first` on: whoever
    /// runs several rules on a turn gives each numbers of its own.
    pub fn count_under(&mut self, first: u32) {
        self.counted_from = first;
    }

    /// Queues the number of `layer_type` at `at` becoming `value`, if
    /// it is still the `seen` the rule read when the write is applied
    /// -- a layer of a bit a cell holds 1 or 0. Refused otherwise, and
    /// nothing happens. Applied, it adds one to the count `counted`,
    /// if one is given: in a group, to count is the group's
    /// ([`Turn::count_if_applied`]).
    pub fn queue_seen(&mut self, layer_type: LayerType, at: CellIndex, seen: u32, value: u32, counted: Option<u32>) {
        debug_assert!(seen != value, "a write that changes nothing");
        debug_assert!(counted.is_none() || self.open.is_none(), "in a group, the group counts");
        let slot = self.slot_of(at.superchunk());
        let count = counted.map_or(NO_COUNT, |place| self.counted_number(place));
        // In a group, marked as its own once it is ended.
        self.outbox.conditional[slot].compared.push(Compared { layer_type, at, seen: seen as u16, value: value as u16, group: NO_GROUP, count });
    }

    /// The number the rule's count `place` is counted under.
    fn counted_number(&self, place: u32) -> u32 {
        let number = self.counted_from + place;
        assert!((number as usize) < COUNTED_WHEN_APPLIED, "count {place} from {}: more than a tick counts when applied", self.counted_from);
        number
    }

    /// Starts a group: the compare-and-writes and the instructions
    /// queued until it is ended ([`Turn::group_end`]) are applied all
    /// or none -- all, if every one of its compare-and-writes finds
    /// its cell as it was seen. They must land in one superchunk: what
    /// applies a superchunk decides it alone.
    pub fn group_start(&mut self) {
        assert!(self.open.is_none(), "a group started in a group");
        let outbox = &*self.outbox;
        self.open = Some(OpenGroup {
            writes: std::array::from_fn(|slot| outbox.writes[slot].len() as u32),
            compared: std::array::from_fn(|slot| outbox.conditional[slot].compared.len() as u32),
            instructions: std::array::from_fn(|slot| outbox.instructions[slot].len() as u32),
        });
        self.outbox.group_counts.clear();
    }

    /// Adds one to the rule's count `place` if the group being queued
    /// is applied.
    pub fn count_if_applied(&mut self, place: u32) {
        assert!(self.open.is_some(), "counted if applied outside a group");
        let number = self.counted_number(place);
        self.outbox.group_counts.push(number);
    }

    /// Ends the group started. One that queued nothing is none.
    pub fn group_end(&mut self) {
        let open = self.open.take().expect("a group ended that was not started");
        let outbox = &mut *self.outbox;
        assert!((0..SLOTS).all(|slot| outbox.writes[slot].len() as u32 == open.writes[slot]), "a write in a group that does not say what was seen");
        let mut landed = (0..SLOTS).filter(|&slot| outbox.conditional[slot].compared.len() as u32 != open.compared[slot] || outbox.instructions[slot].len() as u32 != open.instructions[slot]);
        let Some(slot) = landed.next() else {
            assert!(outbox.group_counts.is_empty(), "a group that only counts");
            return;
        };
        assert!(landed.next().is_none(), "a group landing in two superchunks");
        let conditional = &mut outbox.conditional[slot];
        let group = conditional.groups.len() as u32;
        conditional.compared[open.compared[slot] as usize..].iter_mut().for_each(|write| write.group = group);
        let counts = conditional.counts.len() as u32;
        conditional.counts.extend_from_slice(&outbox.group_counts);
        conditional.groups.push(Group { instructions: (open.instructions[slot], outbox.instructions[slot].len() as u32), counts: (counts, conditional.counts.len() as u32) });
    }
}
