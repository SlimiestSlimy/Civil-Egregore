//! A world from outside the server: every part of it read, and
//! changed only by what keeps its parts agreeing
//! (`docs/server.md`, "What a world holds together").

use crate::{Generation, World, WorldInfo};
use bitplane_manager::{BitmapArena, Write, WritesApplied};
use chunk_storage::{ChunkStorage, LayerType};
use coordinates::SuperchunkIndex;
use entity_manager::{AttributeBlock, Entities, Header, InstructionsApplied};
use simulation::halos::{Halos, Viewport};
use simulation::Simulation;
use std::collections::BTreeMap;

impl World {
    /// What the world is, and the tick it was made or loaded at.
    pub fn info(&self) -> &WorldInfo {
        &self.info
    }

    /// How its superchunks are generated.
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub fn arena(&self) -> &BitmapArena {
        &self.arena
    }

    /// Every superchunk ever made, as stored.
    pub fn storage(&self) -> &ChunkStorage {
        &self.storage
    }

    /// The hot superchunks' entities, at its tick.
    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    /// Its simulation: the hot superchunks' random numbers.
    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }

    /// Each cold superchunk's state, as a save keeps it.
    pub fn cold(&self) -> &BTreeMap<SuperchunkIndex, Vec<u64>> {
        &self.cold
    }

    /// Its halos: the superchunks warming and cooling.
    pub fn halos(&self) -> &Halos {
        &self.halos
    }

    /// Puts `header`'s entity on the world, whole, between two ticks:
    /// what came of it -- lost, if its superchunk is not hot.
    pub fn put_entity(&mut self, header: Header, attributes: &[AttributeBlock]) -> InstructionsApplied {
        self.entities.queue_put(header, attributes);
        self.entities.apply()
    }

    /// Removes `header`'s entity from the world, between two ticks.
    pub fn remove_entity(&mut self, header: &Header) -> InstructionsApplied {
        self.entities.queue_remove(header);
        self.entities.apply()
    }

    /// Applies `writes` to the cells of `layer_type`, between two
    /// ticks: what came of them -- missed, where the cells are not hot.
    pub fn write_cells(&mut self, layer_type: LayerType, writes: impl IntoIterator<Item = Write>) -> WritesApplied {
        writes.into_iter().for_each(|write| self.arena.queue(layer_type, write));
        self.arena.apply()
    }

    /// Keeps the superchunks `viewport` shows hot, if the world's
    /// camera loads them: asked for the next time the halos move.
    pub fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        self.halos.keep_viewport(viewport);
    }

    /// What is wrong between the world's parts, if anything is: the
    /// entities held for other superchunks than the hot ones, a
    /// superchunk both hot and cold, or one made that is neither.
    pub fn broken_invariant(&self) -> Option<String> {
        let hot = self.arena.superchunk_indices();
        let with_entities: Vec<SuperchunkIndex> = self.entities.superchunks().iter().map(|superchunk| superchunk.index()).collect();
        if with_entities != hot {
            return Some(format!("entities are held for {} superchunks, {} are hot", with_entities.len(), hot.len()));
        }
        if let Some(both) = hot.iter().find(|superchunk| self.cold.contains_key(superchunk)) {
            return Some(format!("superchunk {} is hot and cold", both.0));
        }
        let made = self.storage.superchunks().count();
        (made != hot.len() + self.cold.len()).then(|| format!("{made} superchunks made, {} hot and {} cold", hot.len(), self.cold.len()))
    }
}
