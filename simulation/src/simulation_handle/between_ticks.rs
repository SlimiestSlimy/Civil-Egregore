//! The world changed between two ticks: the simulation's own calls,
//! made by whoever runs it for its user -- an entity put or taken, cells
//! written -- and for its loads and saves. Each keeps the world's parts
//! agreeing ([`SimulationHandle::broken_invariant`]).

use super::SimulationHandle;
use bitplane_manager::{Write, WritesApplied};
use chunk_storage::{HeightMap, LayerType, SuperchunkImage};
use coordinates::SuperchunkIndex;
use entity_manager::{AttributeBlock, Header, InstructionsApplied};
use std::path::Path;
use std::sync::Arc;

/// Where a superchunk loaded keeps its image.
pub enum SavedImage {
    /// In memory, as read.
    InMemory(SuperchunkImage),
    /// In the save's folder, read again when wanted.
    InFolder(Arc<Path>),
}

impl SimulationHandle {
    /// Puts `header`'s entity on the world, whole: what came of it --
    /// lost, if its superchunk is not hot. Its cells on any plane are
    /// whoever puts it's to write.
    pub fn add_entity(&mut self, header: Header, attributes: &[AttributeBlock]) -> InstructionsApplied {
        self.entity_store.queue_add(header, attributes);
        self.entity_store.apply()
    }

    /// Removes `header`'s entity from the world. Its cells on any plane
    /// are whoever removes it's to clear.
    pub fn remove_entity(&mut self, header: &Header) -> InstructionsApplied {
        self.entity_store.queue_remove(header.stood_cell);
        self.entity_store.apply()
    }

    /// Applies `writes` to the cells of `layer_type`: what came of them
    /// -- missed, where the cells are not hot.
    pub fn write_cells(&mut self, layer_type: LayerType, writes: impl IntoIterator<Item = Write>) -> WritesApplied {
        writes.into_iter().for_each(|write| self.bitmap_arena.queue_write(layer_type, write));
        self.bitmap_arena.apply()
    }

    /// Has it keep `bytes_kept` bytes of its cold superchunks' images in
    /// memory at most: the rest paged to disk after each tick.
    pub fn keep_cold_pool_within(&mut self, bytes_kept: u64) {
        self.chunk_storage.keep_in_memory(bytes_kept);
    }

    /// Puts `superchunk`, loaded, in the cold pool: its image where
    /// `saved_image` says, and its state -- `state_words`, decoded only
    /// when it turns hot -- kept as a cold one's.
    pub fn put_saved_superchunk(&mut self, superchunk: SuperchunkIndex, saved_image: SavedImage, state_words: Vec<u64>) {
        match saved_image {
            SavedImage::InMemory(image) => self.chunk_storage.insert(superchunk, image),
            SavedImage::InFolder(folder) => self.chunk_storage.insert_on_disk(superchunk, &folder),
        }
        self.cold.insert(superchunk, state_words);
    }

    /// Readies it to be saved: every hot superchunk's changed bitmaps
    /// written back and the ring flushed, so the cold pool's images are
    /// its cells, and a hot one with no cell ever set given an image.
    pub fn ready_for_save(&mut self) {
        self.write_back_and_flush_all();
        for superchunk in self.bitmap_arena.superchunk_indices() {
            if !self.chunk_storage.holds_image(superchunk) {
                self.chunk_storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()));
            }
        }
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
