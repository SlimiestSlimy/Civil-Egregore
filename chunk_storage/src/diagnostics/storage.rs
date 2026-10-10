//! What chunk storage holds: the cold pool's superchunk images and
//! their bytes, and the writeback ring's room.

use crate::ChunkStorage;

/// What chunk storage holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageStats {
    /// Superchunk images in the cold pool.
    pub superchunks: usize,
    /// Those of them paged to disk.
    pub on_disk: usize,
    /// Bytes the images in memory take.
    pub image_bytes: u64,
    /// Bytes the ring holds room for.
    pub ring_bytes: u64,
}

impl StorageStats {
    /// What `storage` holds now.
    pub fn of(storage: &ChunkStorage) -> Self {
        Self { superchunks: storage.cold_pool.len(), on_disk: storage.on_disk(), image_bytes: storage.bytes_in_memory(), ring_bytes: (storage.ring.capacity() * size_of::<u64>()) as u64 }
    }
}
