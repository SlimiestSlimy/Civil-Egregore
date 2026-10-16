//! A world from outside the server: every part of it read, and
//! changed only by what keeps its parts agreeing
//! (`docs/server.md`, "What a world holds together").

use crate::{WorldGeneration, World, WorldInfo};
use bitplane_manager::{BitmapArena, Write, WriteOp, WritesApplied};
use type_registry::COLLISION;
use chunk_storage::{ChunkStorage, LayerType};
use coordinates::SuperchunkIndex;
use entity_manager::{AttributeBlock, WorldEntities, Header, InstructionsApplied};
use simulation::halos::{Halos, Viewport};
use simulation::Simulation;
use std::collections::BTreeMap;

impl World {
    /// What the world is, and the tick it was made or loaded at.
    pub fn world_info(&self) -> &WorldInfo {
        &self.world_info
    }

    /// How its superchunks are generated.
    pub fn world_generation(&self) -> &WorldGeneration {
        &self.world_generation
    }

    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub fn bitmap_arena(&self) -> &BitmapArena {
        &self.bitmap_arena
    }

    /// Every superchunk ever made, as stored.
    pub fn chunk_storage(&self) -> &ChunkStorage {
        &self.chunk_storage
    }

    /// The hot superchunks' entities, at its tick.
    pub fn entity_store(&self) -> &WorldEntities {
        &self.entity_store
    }

    /// Its simulation: the hot superchunks' random numbers.
    pub fn world_simulation(&self) -> &Simulation {
        &self.world_simulation
    }

    /// Each cold superchunk's state, as a save keeps it.
    pub fn cold(&self) -> &BTreeMap<SuperchunkIndex, Vec<u64>> {
        &self.cold
    }

    /// Has it keep `bytes_kept` bytes of its cold superchunks' images
    /// in memory at most, in place of [`crate::COLD_POOL_BYTES_KEPT`]:
    /// the rest paged to disk after each tick.
    pub fn keep_cold_pool_within(&mut self, bytes_kept: u64) {
        self.chunk_storage.keep_in_memory(bytes_kept);
    }

    /// Its halos: the superchunks warming and cooling.
    pub fn halos(&self) -> &Halos {
        &self.halos
    }

    /// Puts `header`'s entity on the world, whole, between two ticks:
    /// what came of it -- lost, if its superchunk is not hot; refused,
    /// if the collision plane holds its cell for another. Put, its
    /// cell is set there (`docs/server.md`, "The collision plane").
    pub fn put_entity(&mut self, header: Header, attributes: &[AttributeBlock]) -> InstructionsApplied {
        if self.bitmap_arena.layer_holds_cell(COLLISION, header.stood_cell) == Ok(true) && self.entity_store.get(header.stood_cell).is_none() {
            return InstructionsApplied { puts_refused: 1, ..InstructionsApplied::default() };
        }
        self.entity_store.queue_put(header, attributes);
        let put_applied = self.entity_store.apply();
        if put_applied.puts > 0 {
            self.write_cells(COLLISION, [Write::cell(header.stood_cell, WriteOp::SetCell)]);
        }
        put_applied
    }

    /// Removes `header`'s entity from the world, between two ticks,
    /// its cell cleared in the collision plane.
    pub fn remove_entity(&mut self, header: &Header) -> InstructionsApplied {
        self.entity_store.queue_remove(header.stood_cell);
        let remove_applied = self.entity_store.apply();
        if remove_applied.entities_removed > 0 {
            self.write_cells(COLLISION, [Write::cell(header.stood_cell, WriteOp::ClearCell)]);
        }
        remove_applied
    }

    /// Applies `writes` to the cells of `layer_type`, between two
    /// ticks: what came of them -- missed, where the cells are not hot.
    pub fn write_cells(&mut self, layer_type: LayerType, writes: impl IntoIterator<Item = Write>) -> WritesApplied {
        writes.into_iter().for_each(|write| self.bitmap_arena.queue_write(layer_type, write));
        self.bitmap_arena.apply()
    }

    /// Keeps the superchunks `viewport` shows hot, if the world's
    /// camera loads them: asked for the next time the halos move.
    pub fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        self.halos.keep_viewport(viewport);
    }

    /// What is wrong between the world's parts, if anything is: the
    /// entities held for other superchunks than the hot ones, a
    /// superchunk both hot and cold, a hot one whose image is paged to
    /// disk, or one made that is neither.
    pub fn broken_invariant(&self) -> Option<String> {
        let hot = self.bitmap_arena.superchunk_indices();
        let with_entities: Vec<SuperchunkIndex> = self.entity_store.superchunks().iter().map(|superchunk| superchunk.superchunk_index()).collect();
        if with_entities != hot {
            return Some(format!("entities are held for {} superchunks, {} are hot", with_entities.len(), hot.len()));
        }
        if let Some(hot_and_cold) = hot.iter().find(|superchunk| self.cold.contains_key(superchunk)) {
            return Some(format!("superchunk {} is hot and cold", hot_and_cold.0));
        }
        if let Some(paged) = hot.iter().find(|&&superchunk| self.chunk_storage.is_on_disk(superchunk)) {
            return Some(format!("superchunk {} is hot, its image paged to disk", paged.0));
        }
        let made_count = self.chunk_storage.superchunks().count();
        (made_count != hot.len() + self.cold.len()).then(|| format!("{made_count} superchunks made, {} hot and {} cold", hot.len(), self.cold.len()))
    }
}
