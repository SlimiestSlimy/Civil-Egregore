//! Civil Egregore's chunks as stored: the cold pool of superchunk
//! images, each one run of words as on disk, and the writeback ring of
//! changed bitmaps that feeds it. Cells are read and changed in the
//! bitplane manager, never here.
//!
//! The design: `docs/chunk_storage.md`; function by function:
//! `docs/chunk_storage_reference.md`. The cold pool and the writeback ring that feeds
//! it (`docs/chunk_storage.md`, "The cold pool and the writeback ring",
//! "Shared images"); its images paged to disk: `paging.rs`.

pub mod disk;
pub mod jobs;
mod layer_codec;
mod paging;
mod superchunk_image;
pub mod wide;
mod writeback_ring;

pub use paging::{read_back, ImageStored};
pub use layer_codec::{BucketKey, LayerCodec};
pub use protocol::type_registry::LayerType;
pub use superchunk_image::{LayerChange, SuperchunkImage};
pub use writeback_ring::WritebackRing;
// Names only the crate's tests use.
#[cfg(test)]
pub(crate) use {crate::bitplane_manager::{WidePlane, Width}, superchunk_image::InvalidImage};

use utilities::coordinates::{ChunkIndex, SuperchunkIndex};
use crate::chunk_storage::paging::{bytes_of, ImageKept, ColdPoolPaging};
use std::sync::Arc;

/// The cold pool and the writeback ring.
pub struct ChunkStorage {
    /// Every superchunk's image, sorted by superchunk index: in
    /// memory shared, so one is read off the tick -- decoded on
    /// another thread -- with nothing copied; or paged to disk. An
    /// image is never changed, only replaced.
    pub(crate) cold_pool: Vec<(SuperchunkIndex, ImageKept)>,
    /// Changed layers, encoded, on their way to the cold pool.
    pub(crate) writeback_ring: WritebackRing,
    /// How the pool is paged to disk, and the bytes it has in memory.
    pub(crate) cold_pool_paging: ColdPoolPaging,
}

impl ChunkStorage {
    /// No superchunk stored, and a ring of `ring_words` words.
    pub fn new(ring_words: usize) -> Self {
        Self { cold_pool: Vec::new(), writeback_ring: WritebackRing::new(ring_words), cold_pool_paging: ColdPoolPaging::never_paged() }
    }

    /// Where `superchunk` is in the cold pool, or where it would go.
    pub(crate) fn find_superchunk(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.cold_pool.binary_search_by_key(&superchunk, |&(stored_superchunk, _)| stored_superchunk)
    }

    /// Puts `image` in the cold pool as `superchunk` (read from disk, or
    /// generated), in memory, in place of the one it had, if any. Its
    /// entries still in the ring are applied to the new image when it
    /// is flushed.
    pub fn insert(&mut self, superchunk: SuperchunkIndex, image: SuperchunkImage) {
        self.cold_pool_paging.bytes_in_memory += bytes_of(&image);
        let image_kept = ImageKept::InMemory(Arc::new(image));
        match self.find_superchunk(superchunk) {
            Ok(pool_index) => {
                if let ImageKept::InMemory(replaced_image) = std::mem::replace(&mut self.cold_pool[pool_index].1, image_kept) {
                    self.cold_pool_paging.bytes_in_memory -= bytes_of(&replaced_image);
                }
            }
            Err(insert_index) => self.cold_pool.insert(insert_index, (superchunk, image_kept)),
        }
    }

    /// Every superchunk the cold pool holds, in Morton order.
    pub fn superchunks(&self) -> impl Iterator<Item = SuperchunkIndex> + '_ {
        self.cold_pool.iter().map(|&(superchunk, _)| superchunk)
    }

    /// Whether the cold pool holds an image of `superchunk`, in memory
    /// or on disk.
    pub fn holds_image(&self, superchunk: SuperchunkIndex) -> bool {
        self.find_superchunk(superchunk).is_ok()
    }

    /// The image of `superchunk`, if the cold pool holds it -- in
    /// memory, as a hot superchunk's is, and any where nothing is
    /// paged. One paged to disk is not lent from here: a panic, the
    /// caller's mistake ([`ChunkStorage::shared_image`] reads it).
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn image(&self, superchunk: SuperchunkIndex) -> Option<&SuperchunkImage> {
        self.find_superchunk(superchunk).ok().map(|pool_index| match &self.cold_pool[pool_index].1 {
            ImageKept::InMemory(image) => &**image,
            ImageKept::OnDisk(_) => panic!("the image of superchunk {:x} is paged to disk: shared_image reads it", superchunk.0),
        })
    }

    /// The image of `superchunk`, if the cold pool holds it, shared: to
    /// read on another thread, as it is now. One paged to disk is read
    /// here, and stays on disk.
    pub fn shared_image(&self, superchunk: SuperchunkIndex) -> Option<Arc<SuperchunkImage>> {
        self.stored_image(superchunk).map(|image_stored| image_stored.image(superchunk))
    }

    /// The encoded layer of `layer_type` in `chunk`, if the cold pool has
    /// one: from its first word to its chunk's end
    /// ([`SuperchunkImage::layer`]). Of an image in memory
    /// ([`ChunkStorage::image`]).
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn layer(&self, chunk: ChunkIndex, layer_type: LayerType) -> Option<&[u64]> {
        self.image(chunk.superchunk())?.layer(chunk.place(), layer_type.type_id())
    }

    /// Writes `encoded_layer` back into the ring as the layer of `layer_type`
    /// in `chunk` (no words: no layer), if it fits -- an empty ring too
    /// small grown until it does: whether it went in. If not, the
    /// superchunk at the ring's tail is to be flushed first
    /// ([`ChunkStorage::take_tail`]).
    pub fn try_write_back(&mut self, chunk: ChunkIndex, layer_type: LayerType, encoded_layer: &[u64]) -> bool {
        while !self.writeback_ring.push(chunk, layer_type.type_id(), encoded_layer) {
            if !self.writeback_ring.is_empty() {
                return false;
            }
            self.writeback_ring.grow_empty(self.writeback_ring.capacity_in_words().max(1) * 2);
        }
        true
    }

    /// [`ChunkStorage::try_write_back`], flushing the superchunk at the
    /// ring's tail here until it fits, and adding each one flushed to
    /// `flushed_superchunks`. Every superchunk was flushed before `encoded_layer` went in,
    /// so its entries left in the ring are this one and later ones.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn write_back(&mut self, chunk: ChunkIndex, layer_type: LayerType, encoded_layer: &[u64], flushed_superchunks: &mut Vec<SuperchunkIndex>) {
        while !self.try_write_back(chunk, layer_type, encoded_layer) {
            let flush = self.take_tail().expect("a full ring has a tail");
            self.insert(flush.superchunk, flush.rewritten_image());
            flushed_superchunks.push(flush.superchunk);
        }
    }

    /// Takes `superchunk`'s entries out of the ring, copied, with its
    /// image as it is, to flush -- here or on another thread -- if it has
    /// any: they are freed from the ring now. Until the image rewritten
    /// with them is put in the cold pool, their newest cells are only in
    /// the bitmap arena, which holds them that long.
    pub fn take_entries(&mut self, superchunk: SuperchunkIndex) -> Option<Flush> {
        let ring_entries = self.writeback_ring.entries_of(superchunk);
        if ring_entries.is_empty() {
            return None;
        }
        let layer_changes = ring_entries.iter().map(|ring_entry| (ring_entry.place, ring_entry.type_id, self.writeback_ring.encoded_layer(ring_entry).to_vec())).collect();
        self.writeback_ring.release_superchunk(superchunk);
        Some(Flush { superchunk, image: self.shared_image(superchunk), layer_changes })
    }

    /// The superchunk at the ring's tail -- the one to flush to make room
    /// -- if the ring holds any change.
    pub fn tail_superchunk(&self) -> Option<SuperchunkIndex> {
        self.writeback_ring.tail_superchunk()
    }

    /// [`ChunkStorage::take_entries`] of the superchunk at the ring's tail.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn take_tail(&mut self) -> Option<Flush> {
        self.take_entries(self.writeback_ring.tail_superchunk()?)
    }

    /// Whether the ring holds any change of `superchunk`.
    pub fn holds_changes(&self, superchunk: SuperchunkIndex) -> bool {
        !self.writeback_ring.entries_of(superchunk).is_empty()
    }

    /// Rewrites the image of `superchunk` with all its entries in the
    /// ring, here, and frees them. Returns whether it had any.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn flush(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Some(flush) = self.take_entries(superchunk) else {
            return false;
        };
        self.insert(superchunk, flush.rewritten_image());
        true
    }

    /// Flushes every superchunk with entries in the ring, each added to
    /// `flushed_superchunks`: the ring is empty after.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn flush_all(&mut self, flushed_superchunks: &mut Vec<SuperchunkIndex>) {
        while let Some(superchunk) = self.writeback_ring.tail_superchunk() {
            self.flush(superchunk);
            flushed_superchunks.push(superchunk);
        }
    }

    /// Whether the ring holds no change.
    pub fn nothing_to_flush(&self) -> bool {
        self.writeback_ring.tail_superchunk().is_none()
    }
}

/// A superchunk's entries taken out of the ring ([`ChunkStorage::take_entries`]),
/// with its image as it was: all a flush needs, so it can be done on any
/// thread.
pub struct Flush {
    /// The superchunk.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub superchunk: SuperchunkIndex,
    /// Its image when the entries were taken, if it had one.
    pub image: Option<Arc<SuperchunkImage>>,
    /// Its entries, oldest first: each layer's chunk place, type's ID and words.
    pub layer_changes: Vec<(usize, u64, Vec<u64>)>,
}

impl Flush {
    /// The image with every change made: of a superchunk with no image
    /// made, on an empty one.
    pub fn rewritten_image(&self) -> SuperchunkImage {
        let image_changes: Vec<LayerChange> = self.layer_changes.iter().map(|(place, type_id, encoded_words)| LayerChange { place: *place, type_id: *type_id, encoded_layer: encoded_words }).collect();
        match &self.image {
            Some(image) => image.rewritten_with(&image_changes),
            None => SuperchunkImage::empty().rewritten_with(&image_changes),
        }
    }
}
