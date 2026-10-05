//! What the arena holds: its superchunks, their allocations and hot
//! bitmaps, and the buckets they keep.

use crate::BitmapArena;

/// What an arena holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaStats {
    /// Superchunks with an allocation in use.
    pub superchunks: usize,
    /// Allocations in use: a layer type over a superchunk each.
    pub allocations: usize,
    /// Hot bitmaps.
    pub hot_bitmaps: usize,
    /// Buckets kept: a hot bitmap with no cell set has none.
    pub buckets: usize,
    /// Bytes of the buckets kept.
    pub bucket_bytes: u64,
}

impl ArenaStats {
    /// What `arena` holds now.
    pub fn of(arena: &BitmapArena) -> Self {
        let buckets = arena.layers().map(|layer| layer.kept.count_ones() as usize).sum();
        let bucket_bytes = arena.layers().map(|layer| size_of_val(&*layer.buckets) as u64).sum();
        Self { superchunks: arena.directory.len(), allocations: arena.allocations(), hot_bitmaps: arena.len(), buckets, bucket_bytes }
    }
}
