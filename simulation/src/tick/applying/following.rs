//! What others queued for an entity that crosses a border follows
//! it: the segments of later authors that name it, re-pointed as the
//! crossing is applied, to the superchunk it came to
//! (`docs/simulation.md`, "An entity over a border").

use super::{Applying, Phase};
use crate::turn::ACROSS;
use coordinates::{CellIndex, SuperchunkIndex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// What became of a segment that names an entity that crossed a
/// border before the segment was come to.
#[derive(Clone)]
pub(super) struct Repointed {
    /// The superchunks it lands in now, by where each is among them
    /// all: all of them, for a segment of a slot's queue; those
    /// besides the ones its outbox says, for an act over a border.
    pub(super) also: Vec<usize>,
    /// How many of the superchunks a segment of a slot's queue lands
    /// in have come to it: it is applied by the last. An act over a
    /// border keeps its own count.
    pub(super) arrived: Arc<AtomicU8>,
}

impl Phase<'_, '_> {
    /// What became of the segment `segment` of the queue `from` of
    /// the outbox at `source`, if an entity it names crossed a border
    /// before it was come to. Nearly always nothing, asked with one
    /// read.
    #[inline]
    pub(super) fn repointed_of(&self, source: u32, from: u32, segment: u32) -> Option<Repointed> {
        if !self.any_repointed.load(Ordering::SeqCst) {
            return None;
        }
        self.repointed.lock().expect("the segments repointed").get(&(source, from, segment)).cloned()
    }

    /// Has what other authors queued for the entities that `crossed`
    /// a border follow them, as the act that had them cross is
    /// applied: every superchunk the act lands in is `held`, each
    /// with where it is among them all, and none has come to a later
    /// author yet. A segment of a later author that names such an
    /// entity lies in the order of the superchunk it left. If it is
    /// of that entity alone it is taken from there and put in the
    /// order of the superchunk the entity came to: the right queue,
    /// and nothing waits. Else it lands in both from now on -- with
    /// whatever else it lands in, if it is an act over a border
    /// already -- and is applied when all have come to it
    /// (`docs/simulation.md`, "An entity over a border").
    pub(super) fn have_follow(&self, crossed: &[(CellIndex, CellIndex)], held: &mut [(usize, &mut Applying)]) {
        let place_of = |superchunk: SuperchunkIndex| self.superchunk_indices.binary_search(&superchunk).expect("a superchunk held");
        for &(stood, to) in crossed {
            let Some(authors) = self.named_by_others.get(&stood) else {
                continue;
            };
            let (left, came_to) = (place_of(stood.superchunk()), place_of(to.superchunk()));
            // Its own place in the order is its name's: only an author after it has anything left to apply.
            let later = |author: u64| author > stood.0 + 1 && authors.contains(&author);
            let leaving = &mut *held.iter_mut().find(|(place, _)| *place == left).expect("the superchunk it left is held").1;
            let mut following = Vec::new();
            let mut repointed = self.repointed.lock().expect("the segments repointed");
            let mut at = leaving.next + 1;
            while let Some(&entry) = leaving.order.get(at) {
                let (author, source, from, segment) = entry;
                at += 1;
                if !later(author) {
                    continue;
                }
                let outbox = &self.outboxes[source as usize];
                if from as usize == ACROSS && self.landed_in(source as usize, outbox.across[segment as usize].lands).0.contains(&Some(came_to)) {
                    // It lands there already.
                    continue;
                }
                match repointed.get_mut(&(source, from, segment)) {
                    Some(already) if already.also.contains(&came_to) => continue,
                    Some(already) => already.also.push(came_to),
                    None if from as usize == ACROSS => {
                        repointed.insert((source, from, segment), Repointed { also: vec![came_to], arrived: Default::default() });
                    }
                    None => {
                        let (queue, segments) = (&outbox.queues[from as usize], &outbox.segments[from as usize]);
                        let some = segments[segment as usize].instructions as usize..segments.get(segment as usize + 1).map_or(queue.len(), |next| next.instructions as usize);
                        // Of that entity alone, it has nothing to do with the superchunk it was queued for any more.
                        let alone = queue.names_alone(some, stood);
                        repointed.insert((source, from, segment), Repointed { also: if alone { vec![came_to] } else { vec![left, came_to] }, arrived: Default::default() });
                        if alone {
                            at -= 1;
                            leaving.order.remove(at);
                        }
                    }
                }
                following.push(entry);
            }
            drop(repointed);
            if !following.is_empty() {
                self.any_repointed.store(true, Ordering::SeqCst);
                let coming = &mut *held.iter_mut().find(|(place, _)| *place == came_to).expect("the superchunk it came to is held").1;
                for entry in following {
                    // After every segment of an author no later than its own: it has come to none of them yet.
                    let at = coming.order.partition_point(|other| other.0 <= entry.0).max(coming.next + 1);
                    coming.order.insert(at, entry);
                }
            }
        }
    }
}
