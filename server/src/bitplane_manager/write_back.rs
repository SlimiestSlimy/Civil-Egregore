//! Dirty buckets written back into chunk storage's ring, the ring
//! flushed, and buckets evicted.

use super::{contains, members, set_membership};
use super::arena::BitmapArena;
use super::superchunk::Superchunk;
use crate::chunk_storage::{BucketKey, ChunkStorage, LayerCodec};
use utilities::coordinates::{ChunkIndex, SuperchunkIndex};

impl BitmapArena {
    /// Takes every dirty bucket over hot `superchunk`: its cells copied
    /// out -- a bitmap's words, times its layer's bits a cell -- with its key, in the arena's order, and marked clean -- one
    /// write-back more on its way, if any. Encoded -- here or off the
    /// tick -- each goes to [`BitmapArena::written_back`], write-backs
    /// in the order taken.
    pub fn take_dirty(&mut self, superchunk: SuperchunkIndex) -> Vec<(BucketKey, Box<[u64]>)> {
        let Ok(entry_index) = self.lookup.superchunk(&self.directory, superchunk) else {
            return Vec::new();
        };
        let mut dirty = Vec::new();
        for allocation in &mut self.directory[entry_index].layers {
            for chunk in members(allocation.chunk_flags.dirty) {
                dirty.push((BucketKey { layer_type: allocation.layer_type, chunk: ChunkIndex::from_superchunk_place(superchunk, chunk) }, allocation.words(chunk).into()));
            }
            allocation.chunk_flags.dirty = 0;
        }
        if !dirty.is_empty() {
            self.directory[entry_index].on_their_way += 1;
        }
        dirty
    }

    /// A write-back of `superchunk` ([`BitmapArena::take_dirty`]) is in
    /// chunk storage's writeback ring: the buckets of `bucket_keys` marked
    /// waiting there -- held until storage tells they were flushed
    /// ([`BitmapArena::flushed_from_ring`]) -- and one write-back fewer on its way.
    pub fn written_back(&mut self, superchunk: SuperchunkIndex, bucket_keys: impl Iterator<Item = BucketKey>) {
        let directory_entry = self.entry_mut(superchunk).expect("a superchunk written back is hot or lingering");
        for bucket_key in bucket_keys {
            // A wide layer is written back as its planes: any of their types names it.
            let layer = directory_entry.layers.iter().position(|layer| layer.layer_type == bucket_key.layer_type || layer.layer_type.holds_bitplane(bucket_key.layer_type)).expect("a layer written back is in use");
            set_membership(&mut directory_entry.layers[layer].chunk_flags.in_ring, bucket_key.chunk.place(), true);
        }
        directory_entry.on_their_way -= 1;
        self.release_unused();
    }

    /// Encodes every dirty bucket over `superchunk` and puts them into
    /// `cold_storage`'s writeback ring, here and now
    /// ([`BitmapArena::take_dirty`], then [`BitmapArena::written_back`]),
    /// the superchunks the ring flushes here to make room
    /// [`BitmapArena::flushed_from_ring`]: how many were written.
    pub fn write_back(&mut self, superchunk: SuperchunkIndex, cold_storage: &mut ChunkStorage, layer_codec: &mut LayerCodec) -> usize {
        let dirty = self.take_dirty(superchunk);
        if dirty.is_empty() {
            return 0;
        }
        let mut flushed_superchunks = Vec::new();
        for (bucket_key, cells) in &dirty {
            let bits_a_cell = bucket_key.layer_type.width_bits();
            for bit_of_cell in 0..bits_a_cell {
                let one_bit_bitmap = crate::chunk_storage::wide::bitmap_of_bit(cells, bits_a_cell, bit_of_cell);
                cold_storage.write_back(bucket_key.chunk, bucket_key.layer_type.bitplane_of(bit_of_cell), layer_codec.encode_layer(&one_bit_bitmap), &mut flushed_superchunks);
            }
        }
        // Flushed before the last bitmap went in: the superchunk written back waits on, all its bitmaps marked.
        self.flushed_from_ring(&flushed_superchunks);
        self.written_back(superchunk, dirty.iter().map(|(written_key, _)| *written_key));
        dirty.len()
    }

    /// `superchunk`, hot or lingering, to change.
    fn entry_mut(&mut self, superchunk: SuperchunkIndex) -> Option<&mut Superchunk> {
        match self.lookup.superchunk(&self.directory, superchunk) {
            Ok(entry_index) => Some(&mut self.directory[entry_index]),
            Err(_) => self.lingering_at(superchunk).ok().map(|lingering_index| &mut self.lingering[lingering_index].superchunk),
        }
    }

    /// Chunk storage has flushed `superchunks` into its cold pool, none
    /// of their changes left in the ring: their buckets no longer wait
    /// there, and the allocations left with no hot bitmap are released.
    pub fn flushed_from_ring(&mut self, superchunks: &[SuperchunkIndex]) {
        superchunks.iter().for_each(|&superchunk| self.leave_ring(superchunk));
        self.release_unused();
    }

    /// Marks every bucket over `superchunk` as no longer waiting in the
    /// ring.
    fn leave_ring(&mut self, superchunk: SuperchunkIndex) {
        if let Some(directory_entry) = self.entry_mut(superchunk) {
            for layer in &mut directory_entry.layers {
                layer.chunk_flags.in_ring = 0;
                layer.let_unused_go();
            }
        }
    }

    /// Releases every allocation with no bucket hot or waiting in the
    /// ring, its buckets with it, every superchunk left with none, and
    /// every lingering superchunk done with.
    pub(crate) fn release_unused(&mut self) {
        self.lingering.retain(|lingering| !lingering.can_be_let_go());
        for directory_entry in &mut self.directory {
            directory_entry.layers.retain(|allocation| allocation.chunk_flags.hot | allocation.chunk_flags.in_ring != 0);
        }
        self.directory.retain(|remaining_entry| !remaining_entry.layers.is_empty());
        self.lookup.forget_last();
    }

    /// Drops `bucket_key`'s bitmap from the hot ones: whether it was hot. Its
    /// bucket stays while it waits in the ring; its allocation, once no
    /// bucket of it is hot or waiting, leaves the directory.
    /// Dropping a dirty bitmap would lose its changes,
    /// and is a bug: write it back first.
    pub fn evict(&mut self, bucket_key: BucketKey) -> bool {
        let Some((allocation_at, chunk)) = self.hot(bucket_key) else {
            return false;
        };
        let allocation = self.layer_at_mut(allocation_at);
        assert!(!contains(allocation.chunk_flags.dirty, chunk), "{bucket_key:?} changed and was not written back");
        set_membership(&mut allocation.chunk_flags.hot, chunk, false);
        allocation.hot_count -= allocation.count(chunk);
        allocation.let_unused_go();
        if allocation.chunk_flags.hot | allocation.chunk_flags.in_ring == 0 {
            let (entry_index, layer) = allocation_at;
            self.directory[entry_index].layers.remove(layer);
            if self.directory[entry_index].layers.is_empty() {
                self.directory.remove(entry_index);
            }
            self.lookup.forget_last();
        }
        true
    }
}
