//! Chunk storage: the cold pool of superchunk images and the writeback
//! ring that feeds it (`../docs/tilesim.md`, "Chunk storage").
//!
//! The bitplane manager decodes layers from the cold pool, and writes
//! the ones it changed back into the ring, encoded. The ring is never
//! read to make a layer hot: a layer with an entry in the ring is still
//! held in the bitmap arena, hot or lingering. When the ring is full, the
//! superchunk at its tail is flushed: its image rewritten once with all
//! its entries, and those entries freed. The bitmap arena is told which
//! superchunks were flushed, since only then may it drop their evicted
//! layers. Images are shared, so one is decoded on another thread while
//! the pool goes on changing (`../docs/chunk_storage.md`, "Shared
//! images").

use coordinates::{ChunkIndex, SuperchunkIndex};
use crate::height_map::HeightMap;
use crate::layer_codec::LayerType;
use crate::superchunk_image::{LayerChange, SuperchunkImage};
use crate::writeback_ring::WritebackRing;
use std::sync::Arc;

/// The cold pool and the writeback ring.
pub struct ChunkStorage {
    /// Every superchunk's image, sorted by superchunk index: shared, so
    /// one is read off the tick -- decoded on another thread -- with
    /// nothing copied. An image is never changed, only replaced.
    pub(crate) cold_pool: Vec<(SuperchunkIndex, Arc<SuperchunkImage>)>,
    /// Changed layers, encoded, on their way to the cold pool.
    pub(crate) ring: WritebackRing,
}

impl ChunkStorage {
    /// No superchunk stored, and a ring of `ring_words` words.
    pub fn new(ring_words: usize) -> Self {
        Self { cold_pool: Vec::new(), ring: WritebackRing::new(ring_words) }
    }

    /// Where `superchunk` is in the cold pool, or where it would go.
    fn find(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.cold_pool.binary_search_by_key(&superchunk, |&(kept, _)| kept)
    }

    /// Puts `image` in the cold pool as `superchunk` (read from disk, or
    /// generated), in place of the one it had, if any. Its entries
    /// still in the ring are applied to the new image when it is
    /// flushed.
    pub fn insert(&mut self, superchunk: SuperchunkIndex, image: SuperchunkImage) {
        match self.find(superchunk) {
            Ok(at) => self.cold_pool[at].1 = Arc::new(image),
            Err(at) => self.cold_pool.insert(at, (superchunk, Arc::new(image))),
        }
    }

    /// Every superchunk the cold pool holds, in Morton order.
    pub fn superchunks(&self) -> impl Iterator<Item = SuperchunkIndex> + '_ {
        self.cold_pool.iter().map(|&(superchunk, _)| superchunk)
    }

    /// The image of `superchunk`, if the cold pool holds it.
    pub fn image(&self, superchunk: SuperchunkIndex) -> Option<&SuperchunkImage> {
        self.find(superchunk).ok().map(|at| &*self.cold_pool[at].1)
    }

    /// The image of `superchunk`, if the cold pool holds it, shared: to
    /// read on another thread, as it is now.
    pub fn shared_image(&self, superchunk: SuperchunkIndex) -> Option<Arc<SuperchunkImage>> {
        self.find(superchunk).ok().map(|at| Arc::clone(&self.cold_pool[at].1))
    }

    /// The encoded layer of `layer_type` in `chunk`, if the cold pool has
    /// one: from its first word to its chunk's end
    /// ([`SuperchunkImage::layer`]).
    pub fn layer(&self, chunk: ChunkIndex, layer_type: LayerType) -> Option<&[u64]> {
        self.image(chunk.superchunk())?.layer(chunk.place(), layer_type)
    }

    /// Writes `encoded` back into the ring as the layer of `layer_type`
    /// in `chunk` (no words: no layer), if it fits -- an empty ring too
    /// small grown until it does: whether it went in. If not, the
    /// superchunk at the ring's tail is to be flushed first
    /// ([`ChunkStorage::take_tail`]).
    pub fn try_write_back(&mut self, chunk: ChunkIndex, layer_type: LayerType, encoded: &[u64]) -> bool {
        while !self.ring.push(chunk, layer_type, encoded) {
            if !self.ring.is_empty() {
                return false;
            }
            self.ring.grow(self.ring.capacity().max(1) * 2);
        }
        true
    }

    /// [`ChunkStorage::try_write_back`], flushing the superchunk at the
    /// ring's tail here until it fits, and adding each one flushed to
    /// `flushed`. Every superchunk was flushed before `encoded` went in,
    /// so its entries left in the ring are this one and later ones.
    pub fn write_back(&mut self, chunk: ChunkIndex, layer_type: LayerType, encoded: &[u64], flushed: &mut Vec<SuperchunkIndex>) {
        while !self.try_write_back(chunk, layer_type, encoded) {
            let flush = self.take_tail().expect("a full ring has a tail");
            self.insert(flush.superchunk, flush.rewritten());
            flushed.push(flush.superchunk);
        }
    }

    /// Takes `superchunk`'s entries out of the ring, copied, with its
    /// image as it is, to flush -- here or on another thread -- if it has
    /// any: they are freed from the ring now. Until the image rewritten
    /// with them is put in the cold pool, their newest cells are only in
    /// the bitmap arena, which holds them that long.
    pub fn take(&mut self, superchunk: SuperchunkIndex) -> Option<Flush> {
        let entries = self.ring.entries_of(superchunk);
        if entries.is_empty() {
            return None;
        }
        let changes = entries.iter().map(|entry| (entry.place, entry.layer_type, self.ring.encoded(entry).to_vec())).collect();
        self.ring.release(superchunk);
        Some(Flush { superchunk, image: self.shared_image(superchunk), changes })
    }

    /// The superchunk at the ring's tail -- the one to flush to make room
    /// -- if the ring holds any change.
    pub fn tail_superchunk(&self) -> Option<SuperchunkIndex> {
        self.ring.tail_superchunk()
    }

    /// [`ChunkStorage::take`] of the superchunk at the ring's tail.
    pub fn take_tail(&mut self) -> Option<Flush> {
        self.take(self.ring.tail_superchunk()?)
    }

    /// Whether the ring holds any change of `superchunk`.
    pub fn holds_changes(&self, superchunk: SuperchunkIndex) -> bool {
        !self.ring.entries_of(superchunk).is_empty()
    }

    /// Rewrites the image of `superchunk` with all its entries in the
    /// ring, here, and frees them. Returns whether it had any.
    pub fn flush(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Some(flush) = self.take(superchunk) else {
            return false;
        };
        self.insert(superchunk, flush.rewritten());
        true
    }

    /// Flushes every superchunk with entries in the ring, each added to
    /// `flushed`: the ring is empty after.
    pub fn flush_all(&mut self, flushed: &mut Vec<SuperchunkIndex>) {
        while let Some(superchunk) = self.ring.tail_superchunk() {
            self.flush(superchunk);
            flushed.push(superchunk);
        }
    }

    /// Whether the ring holds no change.
    pub fn nothing_to_flush(&self) -> bool {
        self.ring.tail_superchunk().is_none()
    }
}

/// A superchunk's entries taken out of the ring ([`ChunkStorage::take`]),
/// with its image as it was: all a flush needs, so it can be done on any
/// thread.
pub struct Flush {
    /// The superchunk.
    pub superchunk: SuperchunkIndex,
    /// Its image when the entries were taken, if it had one.
    pub image: Option<Arc<SuperchunkImage>>,
    /// Its entries, oldest first: each layer's chunk place, type and words.
    pub changes: Vec<(usize, LayerType, Vec<u64>)>,
}

impl Flush {
    /// The image with every change made: a superchunk with no image made,
    /// flat at height 0.
    pub fn rewritten(&self) -> SuperchunkImage {
        let changes: Vec<LayerChange> = self.changes.iter().map(|(place, layer_type, encoded)| LayerChange { place: *place, layer_type: *layer_type, encoded }).collect();
        match &self.image {
            Some(image) => image.rewritten(&changes),
            None => SuperchunkImage::new(&HeightMap::default()).rewritten(&changes),
        }
    }
}
