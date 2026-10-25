//! The cold side: the cold pool of superchunk cold_superchunks, each one run of
//! words as on disk, the writeback ring of changed plane chunks that feeds it,
//! the pool paged to disk past what is kept in memory, a world folder's
//! files, and the jobs that do the slow work off the tick. Cells are read
//! and changed in the hot superchunks, never here.
//!
//! The design: `docs/chunk_storage.md` (docs paused, written for the old
//! server's chunk storage).

pub mod world_folder_files;
pub mod off_tick_jobs;
mod bitmap_codec;
pub(crate) mod cold_pool_paging;
mod cold_superchunk;
pub mod plane_chunk_encoding;
mod cold_writeback_ring;

pub use bitmap_codec::{BitmapCodec, PlaneChunk};
pub use cold_superchunk::{ColdPlaneChunkChange, ColdSuperchunk};
pub use cold_writeback_ring::ColdWritebackRing;
// Names only the crate's tests use.
#[cfg(test)]
pub(crate) use cold_superchunk::InvalidColdSuperchunk;

use utilities::coordinates::SuperchunkIndex;
use crate::cold_storage::cold_pool_paging::{bytes_of, ColdPoolPaging, ColdSuperchunkKept};
use std::sync::Arc;

/// The cold pool and the writeback ring.
pub struct ColdStorage {
    /// Every superchunk's cold superchunk, sorted by superchunk index: in
    /// memory shared, so one is read off the tick -- decoded on
    /// another thread -- with nothing copied; or paged to disk. An
    /// cold superchunk is never changed, only replaced.
    pub(crate) cold_pool: Vec<(SuperchunkIndex, ColdSuperchunkKept)>,
    /// Changed plane chunks, encoded, on their way to the cold pool.
    pub(crate) cold_writeback_ring: ColdWritebackRing,
    /// How the pool is paged to disk, and the bytes it has in memory.
    pub(crate) cold_pool_paging: ColdPoolPaging,
}

impl ColdStorage {
    /// No superchunk stored, and a ring of `ring_words` words.
    pub fn new(ring_words: usize) -> Self {
        Self { cold_pool: Vec::new(), cold_writeback_ring: ColdWritebackRing::new(ring_words), cold_pool_paging: ColdPoolPaging::never_paged() }
    }

    /// Where `superchunk` is in the cold pool, or where it would go.
    pub(crate) fn find_superchunk(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.cold_pool.binary_search_by_key(&superchunk, |&(stored_superchunk, _)| stored_superchunk)
    }

    /// Puts `cold superchunk` in the cold pool as `superchunk` (read from disk, or
    /// generated), in memory, in place of the one it had, if any. Its
    /// entries still in the ring are applied to the new cold superchunk when it
    /// is flushed.
    pub fn insert(&mut self, superchunk: SuperchunkIndex, cold_superchunk: ColdSuperchunk) {
        self.cold_pool_paging.bytes_in_memory += bytes_of(&cold_superchunk);
        let kept = ColdSuperchunkKept::InMemory(Arc::new(cold_superchunk));
        match self.find_superchunk(superchunk) {
            Ok(pool_index) => {
                if let ColdSuperchunkKept::InMemory(replaced) = std::mem::replace(&mut self.cold_pool[pool_index].1, kept) {
                    self.cold_pool_paging.bytes_in_memory -= bytes_of(&replaced);
                }
            }
            Err(insert_index) => self.cold_pool.insert(insert_index, (superchunk, kept)),
        }
    }

    /// Every superchunk the cold pool holds, in Morton order.
    pub fn superchunks(&self) -> impl Iterator<Item = SuperchunkIndex> + '_ {
        self.cold_pool.iter().map(|&(superchunk, _)| superchunk)
    }

    /// Whether the cold pool holds a cold superchunk of `superchunk`, in memory
    /// or on disk.
    pub fn holds_cold_superchunk(&self, superchunk: SuperchunkIndex) -> bool {
        self.find_superchunk(superchunk).is_ok()
    }

    /// The cold superchunk of `superchunk`, if the cold pool holds it -- in
    /// memory, as a hot superchunk's is, and any where nothing is
    /// paged. One paged to disk is not lent from here: a panic, the
    /// caller's mistake ([`ColdStorage::shared_cold_superchunk`] reads it).
    #[cfg(test)]
    pub fn cold_superchunk(&self, superchunk: SuperchunkIndex) -> Option<&ColdSuperchunk> {
        self.find_superchunk(superchunk).ok().map(|pool_index| match &self.cold_pool[pool_index].1 {
            ColdSuperchunkKept::InMemory(cold_superchunk) => &**cold_superchunk,
            ColdSuperchunkKept::OnDisk(_) => panic!("the cold superchunk of superchunk {:x} is paged to disk: shared_cold_superchunk reads it", superchunk.0),
        })
    }

    /// The cold superchunk of `superchunk`, if the cold pool holds it, shared: to
    /// read on another thread, as it is now. One paged to disk is read
    /// here, and stays on disk.
    pub fn shared_cold_superchunk(&self, superchunk: SuperchunkIndex) -> Option<Arc<ColdSuperchunk>> {
        self.kept_cold_superchunk(superchunk).map(|kept| kept.cold_superchunk(superchunk))
    }

    /// The encoded plane chunk of `plane_type` in `chunk`, if the cold pool has
    /// one: from its first word to its chunk's end
    /// ([`ColdSuperchunk::plane chunk`]). Of a cold superchunk in memory
    /// ([`ColdStorage::cold superchunk`]).
    #[cfg(test)]
    pub fn plane_chunk(&self, plane_chunk: PlaneChunk) -> Option<&[u64]> {
        self.cold_superchunk(plane_chunk.chunk.superchunk())?.plane_chunk(plane_chunk.chunk.place(), plane_chunk.plane_type.type_id())
    }

    /// Writes `encoded_plane_chunk` back into the ring as the plane chunk of `plane_type`
    /// in `chunk` (no words: no plane chunk), if it fits -- an empty ring too
    /// small grown until it does: whether it went in. If not, the
    /// superchunk at the ring's tail is to be flushed first
    /// ([`ColdStorage::take_tail`]).
    pub fn try_write_back(&mut self, plane_chunk: PlaneChunk, encoded_plane_chunk: &[u64]) -> bool {
        while !self.cold_writeback_ring.push(plane_chunk.chunk, plane_chunk.plane_type.type_id(), encoded_plane_chunk) {
            if !self.cold_writeback_ring.is_empty() {
                return false;
            }
            self.cold_writeback_ring.grow_empty(self.cold_writeback_ring.capacity_in_words().max(1) * 2);
        }
        true
    }

    /// [`ColdStorage::try_write_back`], flushing the superchunk at the
    /// ring's tail here until it fits, and adding each one flushed to
    /// `flushed_superchunks`. Every superchunk was flushed before `encoded_plane_chunk` went in,
    /// so its entries left in the ring are this one and later ones.
    #[cfg(test)]
    pub fn write_back(&mut self, plane_chunk: PlaneChunk, encoded_plane_chunk: &[u64], flushed_superchunks: &mut Vec<SuperchunkIndex>) {
        while !self.try_write_back(plane_chunk, encoded_plane_chunk) {
            let flush = self.take_tail().expect("a full ring has a tail");
            self.insert(flush.superchunk, flush.rewritten_cold_superchunk());
            flushed_superchunks.push(flush.superchunk);
        }
    }

    /// Takes `superchunk`'s entries out of the ring, copied, with its
    /// cold superchunk as it is, to flush -- here or on another thread -- if it has
    /// any: they are freed from the ring now. Until the cold superchunk rewritten
    /// with them is put in the cold pool, their newest cells are only in
    /// the bitmap arena, which holds them that long.
    pub fn take_entries(&mut self, superchunk: SuperchunkIndex) -> Option<Flush> {
        let ring_entries = self.cold_writeback_ring.entries_of(superchunk);
        if ring_entries.is_empty() {
            return None;
        }
        let plane_chunk_changes = ring_entries.iter().map(|ring_entry| (ring_entry.place, ring_entry.type_id, self.cold_writeback_ring.encoded_plane_chunk(ring_entry).to_vec())).collect();
        self.cold_writeback_ring.release_superchunk(superchunk);
        Some(Flush { superchunk, cold_superchunk: self.shared_cold_superchunk(superchunk), plane_chunk_changes })
    }

    /// The superchunk at the ring's tail -- the one to flush to make room
    /// -- if the ring holds any change.
    pub fn tail_superchunk(&self) -> Option<SuperchunkIndex> {
        self.cold_writeback_ring.tail_superchunk()
    }

    /// [`ColdStorage::take_entries`] of the superchunk at the ring's tail.
    #[cfg(test)]
    pub fn take_tail(&mut self) -> Option<Flush> {
        self.take_entries(self.cold_writeback_ring.tail_superchunk()?)
    }

    /// Whether the ring holds any change of `superchunk`.
    pub fn holds_changes(&self, superchunk: SuperchunkIndex) -> bool {
        !self.cold_writeback_ring.entries_of(superchunk).is_empty()
    }

    /// Rewrites the cold superchunk of `superchunk` with all its entries in the
    /// ring, here, and frees them. Returns whether it had any.
    #[cfg(test)]
    pub fn flush(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Some(flush) = self.take_entries(superchunk) else {
            return false;
        };
        self.insert(superchunk, flush.rewritten_cold_superchunk());
        true
    }

    /// Flushes every superchunk with entries in the ring, each added to
    /// `flushed_superchunks`: the ring is empty after.
    #[cfg(test)]
    pub fn flush_all(&mut self, flushed_superchunks: &mut Vec<SuperchunkIndex>) {
        while let Some(superchunk) = self.cold_writeback_ring.tail_superchunk() {
            self.flush(superchunk);
            flushed_superchunks.push(superchunk);
        }
    }

    /// Whether the ring holds no change.
    pub fn nothing_to_flush(&self) -> bool {
        self.cold_writeback_ring.tail_superchunk().is_none()
    }
}

/// A superchunk's entries taken out of the ring ([`ColdStorage::take_entries`]),
/// with its cold superchunk as it was: all a flush needs, so it can be done on any
/// thread.
pub struct Flush {
    /// The superchunk.
    pub superchunk: SuperchunkIndex,
    /// Its cold superchunk when the entries were taken, if it had one.
    pub cold_superchunk: Option<Arc<ColdSuperchunk>>,
    /// Its entries, oldest first: each plane chunk's chunk place, type's ID and words.
    pub plane_chunk_changes: Vec<(usize, u64, Vec<u64>)>,
}

impl Flush {
    /// The cold superchunk with every change made: of a superchunk with no cold superchunk
    /// made, on an empty one.
    pub fn rewritten_cold_superchunk(&self) -> ColdSuperchunk {
        let plane_chunk_changes_in: Vec<ColdPlaneChunkChange> = self.plane_chunk_changes.iter().map(|(place, type_id, encoded_words)| ColdPlaneChunkChange { place: *place, type_id: *type_id, encoded_plane_chunk: encoded_words }).collect();
        match &self.cold_superchunk {
            Some(cold_superchunk) => cold_superchunk.rewritten_with(&plane_chunk_changes_in),
            None => ColdSuperchunk::empty().rewritten_with(&plane_chunk_changes_in),
        }
    }
}
