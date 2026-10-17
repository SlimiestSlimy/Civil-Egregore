//! Every superchunk's entities together: the world's, with what is
//! queued to be put and removed.

use crate::attributes::{AttributeBlock, DefaultAttributes};
use crate::entity::{EntityRef, Header};
use crate::instructions::{Instructions, InstructionsApplied};
use super::SuperchunkEntities;
use super::entity_reader::EntityReader;
use coordinates::{CellIndex, SuperchunkIndex};
use std::sync::Arc;
use type_registry::FrozenTypes;

/// Every superchunk's entities, in the bitmap arena's order -- by Morton
/// index -- and the tick the world is at: the next to run.
#[derive(Default)]
pub struct WorldEntities {
    /// The tick about to run.
    current_tick: u64,
    /// The superchunks, by Morton index.
    superchunks: Vec<SuperchunkEntities>,
    /// Instructions queued outside a tick, applied by [`WorldEntities::apply`].
    queued_instructions: Instructions,
    /// The default attributes of the types the world runs with: each
    /// superchunk's too.
    default_attributes: Arc<DefaultAttributes>,
}

impl WorldEntities {
    /// None, at tick 0, no type of entity with a default attribute.
    pub fn new() -> Self {
        Self::default()
    }

    /// None, at tick `current_tick`, of a world running with
    /// `frozen_types`: what a world's entities start as, and a save's are
    /// put back into.
    pub fn for_types<RuleCode>(frozen_types: &FrozenTypes<RuleCode>, current_tick: u64) -> Self {
        Self { current_tick, default_attributes: Arc::new(DefaultAttributes::of_types(frozen_types)), ..Self::default() }
    }

    /// The default attributes of the types the world runs with.
    pub fn default_attributes(&self) -> &DefaultAttributes {
        &self.default_attributes
    }

    /// The tick about to run.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
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

    /// The entity standing on `stood_cell`, if one does.
    pub fn get(&self, stood_cell: CellIndex) -> Option<EntityRef<'_>> {
        self.superchunk(stood_cell.superchunk())?.get(stood_cell)
    }

    /// Keeps exactly `superchunk_indices`, sorted:
    /// those it lacked added empty, those not among them dropped with
    /// their entities -- how many entities were dropped. Keeping them in
    /// chunk storage when their bitmaps go cold is work to come.
    pub fn keep_superchunks(&mut self, superchunk_indices: &[SuperchunkIndex]) -> usize {
        if self.superchunks.len() == superchunk_indices.len() && self.superchunks.iter().zip(superchunk_indices).all(|(kept_superchunk, &wanted_index)| kept_superchunk.superchunk_index == wanted_index) {
            return 0;
        }
        let mut had_superchunks = std::mem::take(&mut self.superchunks).into_iter().peekable();
        let mut dropped_entities = 0;
        for &wanted_index in superchunk_indices {
            while let Some(superchunk) = had_superchunks.next_if(|superchunk| superchunk.superchunk_index < wanted_index) {
                dropped_entities += superchunk.len();
            }
            let superchunk = had_superchunks.next_if(|superchunk| superchunk.superchunk_index == wanted_index).unwrap_or_else(|| SuperchunkEntities::new(wanted_index, Arc::clone(&self.default_attributes)));
            self.superchunks.push(superchunk);
        }
        dropped_entities + had_superchunks.map(|superchunk| superchunk.len()).sum::<usize>()
    }

    /// Queues making `header`'s entity, with `attributes` sorted by
    /// type, outside a tick: setting up, say. It wakes at its wake
    /// tick, the tick about to run or later. Not made, an entity
    /// standing on its cell.
    pub fn queue_add(&mut self, header: Header, attributes: &[AttributeBlock]) {
        self.queued_instructions.add(header, attributes);
    }

    /// Queues removing the entity standing on `stood_cell`, outside a tick.
    pub fn queue_remove(&mut self, stood_cell: CellIndex) {
        self.queued_instructions.remove(stood_cell);
    }

    /// How many instructions are queued.
    pub fn queued_count(&self) -> usize {
        self.queued_instructions.len()
    }

    /// Applies the instructions queued, in order, and empties the queue:
    /// an entity put in a superchunk not kept here is lost, one put on a
    /// cell another stands on refused.
    pub fn apply(&mut self) -> InstructionsApplied {
        let mut instructions_applied = InstructionsApplied::default();
        self.queued_instructions.apply(&mut self.superchunks, self.current_tick, &mut instructions_applied);
        self.queued_instructions.clear();
        let current_tick = self.current_tick;
        self.superchunks.iter_mut().for_each(|superchunk| superchunk.sort_wakes(current_tick));
        self.superchunks.iter_mut().for_each(|superchunk| superchunk.names_anew());
        instructions_applied
    }

    /// Every entity, superchunk by superchunk.
    pub fn iter(&self) -> impl Iterator<Item = EntityRef<'_>> {
        self.superchunks.iter().flat_map(SuperchunkEntities::iter)
    }

    /// The tick just run is over: the next is about to run.
    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }
}
