//! Dirty buckets written back into chunk storage's ring, the ring
//! flushed, and buckets evicted.

use super::{contains, members, put};
use super::arena::BitmapArena;
use super::superchunk::Superchunk;
use chunk_storage::{BucketKey, ChunkStorage, LayerCodec};
use coordinates::{ChunkIndex, SuperchunkIndex};

impl BitmapArena {
    /// Takes every dirty bucket over hot `superchunk`: its cells copied
    /// out -- a bitmap's words, times its layer's bits a cell -- with its key, in the arena's order, and marked clean -- one
    /// write-back more on its way, if any. Encoded -- here or off the
    /// tick -- each goes to [`BitmapArena::written_back`], write-backs
    /// in the order taken.
    pub fn take_dirty(&mut self, superchunk: SuperchunkIndex) -> Vec<(BucketKey, Box<[u64]>)> {
        let Ok(entry) = self.lookup.superchunk(&self.directory, superchunk) else {
            return Vec::new();
        };
        let mut dirty = Vec::new();
        for allocation in &mut self.directory[entry].layers {
            for chunk in members(allocation.flags.dirty) {
                dirty.push((BucketKey { layer_type: allocation.layer_type, chunk: ChunkIndex::of(superchunk, chunk) }, allocation.words(chunk).into()));
            }
            allocation.flags.dirty = 0;
        }
        if !dirty.is_empty() {
            self.directory[entry].on_their_way += 1;
        }
        dirty
    }

    /// A write-back of `superchunk` ([`BitmapArena::take_dirty`]) is in
    /// chunk storage's writeback ring: the buckets of `keys` marked
    /// waiting there -- held until storage tells they were flushed
    /// ([`BitmapArena::flushed`]) -- and one write-back fewer on its way.
    pub fn written_back(&mut self, superchunk: SuperchunkIndex, keys: impl Iterator<Item = BucketKey>) {
        let entry = self.entry_mut(superchunk).expect("a superchunk written back is hot or lingering");
        for key in keys {
            // A wide layer is written back as its planes: any of their types names it.
            let layer = entry.layers.iter().position(|layer| layer.layer_type == key.layer_type || layer.layer_type.holds(key.layer_type)).expect("a layer written back is in use");
            put(&mut entry.layers[layer].flags.in_ring, key.chunk.place(), true);
        }
        entry.on_their_way -= 1;
        self.release_unused();
    }

    /// Encodes every dirty bucket over `superchunk` and puts them into
    /// `storage`'s writeback ring, here and now
    /// ([`BitmapArena::take_dirty`], then [`BitmapArena::written_back`]),
    /// the superchunks the ring flushes here to make room
    /// [`BitmapArena::flushed`]: how many were written.
    pub fn write_back(&mut self, superchunk: SuperchunkIndex, storage: &mut ChunkStorage, codec: &mut LayerCodec) -> usize {
        let dirty = self.take_dirty(superchunk);
        if dirty.is_empty() {
            return 0;
        }
        let mut flushed = Vec::new();
        for (key, cells) in &dirty {
            let bits = key.layer_type.bits();
            for bit in 0..bits {
                let plane = chunk_storage::wide::plane(cells, bits, bit);
                storage.write_back(key.chunk, key.layer_type.plane(bit), codec.encode_layer(&plane), &mut flushed);
            }
        }
        // Flushed before the last bitmap went in: the superchunk written back waits on, all its bitmaps marked.
        self.flushed(&flushed);
        self.written_back(superchunk, dirty.iter().map(|(key, _)| *key));
        dirty.len()
    }

    /// `superchunk`, hot or lingering, to change.
    fn entry_mut(&mut self, superchunk: SuperchunkIndex) -> Option<&mut Superchunk> {
        match self.lookup.superchunk(&self.directory, superchunk) {
            Ok(entry) => Some(&mut self.directory[entry]),
            Err(_) => self.lingering_at(superchunk).ok().map(|at| &mut self.lingering[at].superchunk),
        }
    }

    /// Chunk storage has flushed `superchunks` into its cold pool, none
    /// of their changes left in the ring: their buckets no longer wait
    /// there, and the allocations left with no hot bitmap are released.
    pub fn flushed(&mut self, superchunks: &[SuperchunkIndex]) {
        superchunks.iter().for_each(|&superchunk| self.leave_ring(superchunk));
        self.release_unused();
    }

    /// Marks every bucket over `superchunk` as no longer waiting in the
    /// ring.
    fn leave_ring(&mut self, superchunk: SuperchunkIndex) {
        if let Some(entry) = self.entry_mut(superchunk) {
            for layer in &mut entry.layers {
                layer.flags.in_ring = 0;
                layer.let_unused_go();
            }
        }
    }

    /// Releases every allocation with no bucket hot or waiting in the
    /// ring, its buckets with it, every superchunk left with none, and
    /// every lingering superchunk done with.
    pub(crate) fn release_unused(&mut self) {
        self.lingering.retain(|lingering| !lingering.done());
        for entry in &mut self.directory {
            entry.layers.retain(|allocation| allocation.flags.hot | allocation.flags.in_ring != 0);
        }
        self.directory.retain(|entry| !entry.layers.is_empty());
        self.lookup.forget();
    }

    /// Drops `key`'s bitmap from the hot ones: whether it was hot. Its
    /// bucket stays while it waits in the ring; its allocation, once no
    /// bucket of it is hot or waiting, leaves the directory.
    /// Dropping a dirty bitmap would lose its changes,
    /// and is a bug: write it back first.
    pub fn evict(&mut self, key: BucketKey) -> bool {
        let Some((at, chunk)) = self.hot(key) else {
            return false;
        };
        let allocation = self.at_mut(at);
        assert!(!contains(allocation.flags.dirty, chunk), "{key:?} changed and was not written back");
        put(&mut allocation.flags.hot, chunk, false);
        allocation.hot_count -= allocation.count(chunk);
        allocation.let_unused_go();
        if allocation.flags.hot | allocation.flags.in_ring == 0 {
            let (entry, layer) = at;
            self.directory[entry].layers.remove(layer);
            if self.directory[entry].layers.is_empty() {
                self.directory.remove(entry);
            }
            self.lookup.forget();
        }
        true
    }
}
