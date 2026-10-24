//! A superchunk being generated ([`GeneratingSuperchunk`]): the arena's
//! own form of its cells and the store's own of its entities, lent to the
//! world's generation on the warming's job, off the tick, and made hot
//! whole as it turns hot. Nothing sees it till then, so nothing is
//! compared. It is written through the arena's and the store's own APIs:
//! a write outside it is not applied, an entity on a taken cell not made.

use crate::bitplane_manager::Superchunk;
use crate::chunk_storage::LayerType;
use crate::entity_manager::{DefaultAttributes, EntityBeingMade, EntityType, FirstWake, SuperchunkEntities};
use std::sync::Arc;
use utilities::coordinates::{CellIndex, SuperchunkIndex};

/// A superchunk being generated: its cells and its entities as the arena
/// and the store keep a superchunk's, and the tick it turns hot at.
pub struct GeneratingSuperchunk {
    /// Its cells: every layer, every chunk hot, nothing set till written.
    cells: Superchunk,
    /// Its entities.
    entities: SuperchunkEntities,
    /// The tick it turns hot at: what its entities are made for.
    first_hot_tick: u64,
}

impl GeneratingSuperchunk {
    /// `superchunk` to generate, turning hot at `first_hot_tick`: a layer
    /// of each of `layer_types`, of a world whose default attributes are
    /// `default_attributes`, nothing set and no entity.
    pub(crate) fn new(superchunk: SuperchunkIndex, layer_types: &[LayerType], first_hot_tick: u64, default_attributes: Arc<DefaultAttributes>) -> Self {
        Self { cells: Superchunk::to_generate(superchunk, layer_types), entities: SuperchunkEntities::new(superchunk, default_attributes), first_hot_tick }
    }

    /// Which superchunk it is.
    pub fn superchunk_index(&self) -> SuperchunkIndex {
        self.cells.superchunk_index()
    }

    /// The tick it turns hot at: what times its entities keep, as ticks,
    /// are counted from.
    pub fn first_hot_tick(&self) -> u64 {
        self.first_hot_tick
    }

    /// Its cells, to read ([`Superchunk::value_at`]).
    pub fn cells(&self) -> &Superchunk {
        &self.cells
    }

    /// Its cells, to write ([`Superchunk::apply`]).
    pub fn cells_mut(&mut self) -> &mut Superchunk {
        &mut self.cells
    }

    /// Its entities, to read ([`SuperchunkEntities::occupied`]).
    pub fn entities(&self) -> &SuperchunkEntities {
        &self.entities
    }

    /// An entity to make on `cell`, for the tick it turns hot at.
    pub fn new_entity(&mut self, entity_type: EntityType, cell: CellIndex, first_wake: FirstWake) -> EntityBeingMade<'_> {
        self.entities.new_entity(self.first_hot_tick, entity_type, cell, first_wake)
    }

    /// It made, for the arena and the store to take as it turns hot.
    pub(crate) fn into_parts(self) -> (Superchunk, SuperchunkEntities) {
        (self.cells, self.entities)
    }
}
