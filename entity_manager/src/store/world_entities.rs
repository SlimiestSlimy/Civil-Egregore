//! Every superchunk's entities together: the world's, with what is
//! queued to be put and removed.

use crate::entity::{Attribute, EntityId, EntityRef, Header};
use crate::instructions::{Instructions, InstructionsApplied};
use super::SuperchunkEntities;
use super::entity_reader::EntityReader;
use coordinates::{CellIndex, SuperchunkIndex};

/// Every superchunk's entities, in the bitmap arena's order -- by Morton
/// index -- and the tick the world is at: the next to run.
#[derive(Default)]
pub struct Entities {
    /// The tick about to run.
    now: u64,
    /// The superchunks, by Morton index.
    superchunks: Vec<SuperchunkEntities>,
    /// Instructions queued outside a tick, applied by [`Entities::apply`].
    queued: Instructions,
}

impl Entities {
    /// None, at tick 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// The tick about to run.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// None, at tick `now`: what a save's entities are put back into.
    pub fn at_tick(now: u64) -> Self {
        Self { now, ..Self::default() }
    }

    /// How many entities there are.
    pub fn len(&self) -> usize {
        self.superchunks.iter().map(SuperchunkEntities::len).sum()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The superchunks, by Morton index.
    pub fn superchunks(&self) -> &[SuperchunkEntities] {
        &self.superchunks
    }

    /// The superchunks, by Morton index, to change.
    pub fn superchunks_mut(&mut self) -> &mut [SuperchunkEntities] {
        &mut self.superchunks
    }

    /// `superchunk`'s entities, if kept here.
    pub fn superchunk(&self, superchunk: SuperchunkIndex) -> Option<&SuperchunkEntities> {
        EntityReader::new(&self.superchunks).superchunk(superchunk)
    }

    /// The entity whose ID is `id`, standing on `at`, if kept here.
    pub fn get(&self, id: EntityId, at: CellIndex) -> Option<EntityRef<'_>> {
        self.superchunk(at.superchunk())?.get(id, at)
    }

    /// Keeps exactly `superchunk_indices`, sorted:
    /// those it lacked added empty, those not among them dropped with
    /// their entities -- how many entities were dropped. Keeping them in
    /// chunk storage when their bitmaps go cold is work to come.
    pub fn align(&mut self, superchunk_indices: &[SuperchunkIndex]) -> usize {
        if self.superchunks.len() == superchunk_indices.len() && self.superchunks.iter().zip(superchunk_indices).all(|(kept, &index)| kept.index == index) {
            return 0;
        }
        let mut had = std::mem::take(&mut self.superchunks).into_iter().peekable();
        let mut dropped = 0;
        for &index in superchunk_indices {
            while let Some(superchunk) = had.next_if(|superchunk| superchunk.index < index) {
                dropped += superchunk.len();
            }
            let superchunk = had.next_if(|superchunk| superchunk.index == index).unwrap_or_else(|| SuperchunkEntities::new(index));
            self.superchunks.push(superchunk);
        }
        dropped + had.map(|superchunk| superchunk.len()).sum::<usize>()
    }

    /// Queues putting `header`'s entity -- made, or changed where it
    /// stands -- with `attributes` sorted by type, outside a tick:
    /// setting up, say. It wakes at its wake tick, the tick about to run
    /// or later. One to stand elsewhere is removed, and put there.
    pub fn queue_put(&mut self, header: Header, attributes: &[Attribute]) {
        self.queued.put(header, header.at, attributes);
    }

    /// Queues removing `header`'s entity, outside a tick.
    pub fn queue_remove(&mut self, header: &Header) {
        self.queued.remove(header.id, header.at);
    }

    /// How many instructions are queued.
    pub fn queued(&self) -> usize {
        self.queued.len()
    }

    /// Applies the instructions queued, in order, and empties the queue:
    /// an entity put in a superchunk not kept here is lost, one put on a
    /// cell another stands on refused.
    pub fn apply(&mut self) -> InstructionsApplied {
        let mut applied = InstructionsApplied::default();
        self.queued.apply(&mut self.superchunks, self.now, &mut applied);
        self.queued.clear();
        let now = self.now;
        self.superchunks.iter_mut().for_each(|superchunk| superchunk.sort_wakes(now));
        applied
    }

    /// Every entity, superchunk by superchunk.
    pub fn iter(&self) -> impl Iterator<Item = EntityRef<'_>> {
        self.superchunks.iter().flat_map(SuperchunkEntities::iter)
    }

    /// The tick just run is over: the next is about to run.
    pub fn advance(&mut self) {
        self.now += 1;
    }
}
