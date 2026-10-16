//! What others queued for an entity that crosses a border follows
//! it: the segments of later authors that name it, re-pointed as the
//! crossing is applied, to the superchunk it came to
//! (`docs/simulation.md`, "An entity over a border").

use super::{ApplyingSuperchunk, Phase};
use crate::turn::{Outbox, ACROSS_QUEUE};
use coordinates::{CellIndex, SuperchunkIndex};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// What became of a segment that names an entity that crossed a
/// border before the segment was come to.
#[derive(Clone)]
pub(super) struct RepointedSegment {
    /// The superchunks it lands in now, by where each is among them
    /// all: all of them, for a segment of a slot's queue; those
    /// besides the ones its outbox says, for an act over a border.
    pub(super) also_lands_in: Vec<usize>,
    /// How many of the superchunks a segment of a slot's queue lands
    /// in have come to it: it is applied by the last. An act over a
    /// border keeps its own count.
    pub(super) arrived_count: Arc<AtomicU8>,
}

/// Where what is queued for an entity goes when the entity is not
/// where the tick found it: all the second phase keeps of entities
/// named by others and of segments re-pointed after them, in one
/// place. An instruction names an entity by the cell the tick found
/// it on; this is what has that name reach the entity in whichever
/// superchunk it stands by then.
pub(super) struct EntityFollowing {
    /// The entities an author that is not the entity itself named
    /// this tick, each with those authors: by the cell the tick found
    /// it on. What is looked at as an entity crosses a border.
    named_by_others: HashMap<CellIndex, Vec<u64>>,
    /// Whether any segment was repointed this tick: read before the
    /// map is.
    any_repointed: AtomicBool,
    /// The segments an entity that crossed a border had follow it, by
    /// outbox, queue and segment.
    repointed_segments: Mutex<HashMap<(u32, u32, u32), RepointedSegment>>,
}

impl EntityFollowing {
    /// Nothing re-pointed yet, of a tick whose `outboxes` say who
    /// named whom.
    pub(super) fn of_what_was_queued(outboxes: &[Outbox]) -> Self {
        let mut named_by_others: HashMap<CellIndex, Vec<u64>> = HashMap::new();
        for &(named_cell, author) in outboxes.iter().flat_map(|outbox| &outbox.named_by_others) {
            named_by_others.entry(named_cell).or_default().push(author);
        }
        Self { named_by_others, any_repointed: AtomicBool::new(false), repointed_segments: Mutex::new(HashMap::new()) }
    }

    /// Whether no author named an entity other than itself this
    /// tick: then nothing follows any entity.
    pub(super) fn nobody_named_another(&self) -> bool {
        self.named_by_others.is_empty()
    }

    /// What became of the segment `segment_number` of the queue `from_slot` of
    /// the outbox at `source_arena_index`, if an entity it names crossed a border
    /// before it was come to. Nearly always nothing, asked with one
    /// read.
    #[inline]
    pub(super) fn repointed_of(&self, source_arena_index: u32, from_slot: u32, segment_number: u32) -> Option<RepointedSegment> {
        if !self.any_repointed.load(Ordering::SeqCst) {
            return None;
        }
        self.repointed_segments.lock().expect("the segments repointed").get(&(source_arena_index, from_slot, segment_number)).cloned()
    }
}

impl Phase<'_, '_> {

    /// Has what other authors queued for the entities that `crossed_entities`
    /// a border follow them, as the act that had them cross is
    /// applied: every superchunk the act lands in is `locked_applying`, each
    /// with where it is among them all, and none has come to a later
    /// author yet. A segment of a later author that names such an
    /// entity lies in the order of the superchunk it left. If it is
    /// of that entity alone it is taken from there and put in the
    /// order of the superchunk the entity came to: the right queue,
    /// and nothing waits. Else it lands in both from now on -- with
    /// whatever else it lands in, if it is an act over a border
    /// already -- and is applied when all have come to it
    /// (`docs/simulation.md`, "An entity over a border").
    pub(super) fn have_follow(&self, crossed_entities: &[(CellIndex, CellIndex)], locked_applying: &mut [(usize, &mut ApplyingSuperchunk)]) {
        let place_of = |superchunk: SuperchunkIndex| self.superchunk_indices.binary_search(&superchunk).expect("a superchunk held");
        for &(stood_cell, came_to_cell) in crossed_entities {
            let Some(authors) = self.entity_following.named_by_others.get(&stood_cell) else {
                continue;
            };
            let (left_arena_index, came_to) = (place_of(stood_cell.superchunk()), place_of(came_to_cell.superchunk()));
            // Its own place in the order is its name's: only an author after it has anything left to apply.
            let is_later_author = |author: u64| author > stood_cell.0 + 1 && authors.contains(&author);
            let leaving_applying = &mut *locked_applying.iter_mut().find(|(place, _)| *place == left_arena_index).expect("the superchunk it left is held").1;
            let mut following_entries = Vec::new();
            let mut repointed_guard = self.entity_following.repointed_segments.lock().expect("the segments repointed");
            let mut order_index = leaving_applying.next_segment + 1;
            while let Some(&queued_entry) = leaving_applying.segment_order.get(order_index) {
                let (author, source_arena_index, from_slot, segment_number) = queued_entry;
                order_index += 1;
                if !is_later_author(author) {
                    continue;
                }
                let outbox = &self.outboxes[source_arena_index as usize];
                if from_slot as usize == ACROSS_QUEUE && self.landed_in(source_arena_index as usize, outbox.across_acts[segment_number as usize].lands_in_slots).contains(&Some(came_to)) {
                    // It lands there already.
                    continue;
                }
                match repointed_guard.get_mut(&(source_arena_index, from_slot, segment_number)) {
                    Some(already_landing) if already_landing.also_lands_in.contains(&came_to) => continue,
                    Some(already_repointed) => already_repointed.also_lands_in.push(came_to),
                    None if from_slot as usize == ACROSS_QUEUE => {
                        repointed_guard.insert((source_arena_index, from_slot, segment_number), RepointedSegment { also_lands_in: vec![came_to], arrived_count: Default::default() });
                    }
                    None => {
                        let (segment_queue, queue_segments) = (&outbox.instruction_queues[from_slot as usize], &outbox.author_segments[from_slot as usize]);
                        let segment_instructions = queue_segments[segment_number as usize].instructions as usize..queue_segments.get(segment_number as usize + 1).map_or(segment_queue.instructions.len(), |next_start| next_start.instructions as usize);
                        // Of that entity alone, it has nothing to do with the superchunk it was queued for any more.
                        let names_it_alone = segment_queue.names_alone(segment_instructions, stood_cell);
                        repointed_guard.insert((source_arena_index, from_slot, segment_number), RepointedSegment { also_lands_in: if names_it_alone { vec![came_to] } else { vec![left_arena_index, came_to] }, arrived_count: Default::default() });
                        if names_it_alone {
                            order_index -= 1;
                            leaving_applying.segment_order.remove(order_index);
                        }
                    }
                }
                following_entries.push(queued_entry);
            }
            drop(repointed_guard);
            if !following_entries.is_empty() {
                self.entity_following.any_repointed.store(true, Ordering::SeqCst);
                let coming_applying = &mut *locked_applying.iter_mut().find(|(place, _)| *place == came_to).expect("the superchunk it came to is held").1;
                for following_entry in following_entries {
                    // After every segment of an author no later than its own: it has come to none of them yet.
                    let insert_index = coming_applying.segment_order.partition_point(|ordered_entry| ordered_entry.0 <= following_entry.0).max(coming_applying.next_segment + 1);
                    coming_applying.segment_order.insert(insert_index, following_entry);
                }
            }
        }
    }
}
