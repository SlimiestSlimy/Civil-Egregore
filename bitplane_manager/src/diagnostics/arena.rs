//! What the arena holds: its superchunks, their allocations and hot
//! bitmaps, and the bytes of the blocks they live in.

use crate::BitmapArena;
use allocator::diagnostics::block_pool::BlockPoolStats;

/// What an arena holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaStats {
    /// Superchunks with an allocation in use.
    pub superchunks: usize,
    /// Allocations in use: a layer type over a superchunk each.
    pub allocations: usize,
    /// Hot bitmaps.
    pub hot_bitmaps: usize,
    /// The blocks the arena has made, in use or released, over its
    /// block pools: one a width.
    pub blocks_made: usize,
    /// Bytes of every block made.
    pub bytes_made: u64,
    /// Bytes of the blocks in use.
    pub bytes_in_use: u64,
}

impl ArenaStats {
    /// What `arena` holds now.
    pub fn of(arena: &BitmapArena) -> Self {
        let pools = arena.block_pools.each_ref().map(BlockPoolStats::of);
        Self { superchunks: arena.directory.len(), allocations: arena.allocations(), hot_bitmaps: arena.len(), blocks_made: pools.iter().map(|pool| pool.made).sum(), bytes_made: pools.iter().map(BlockPoolStats::bytes_made).sum(), bytes_in_use: arena.layers().map(|layer| size_of_val(&*layer.block) as u64).sum() }
    }

    /// Bytes of the blocks in use.
    pub fn bytes_in_use(&self) -> u64 {
        self.bytes_in_use
    }
}
