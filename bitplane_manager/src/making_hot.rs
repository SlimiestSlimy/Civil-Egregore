//! Bitmaps made hot and cold: a bucket, a chunk's layers, a superchunk
//! whole -- and a superchunk gone cold kept lingering.

use super::{COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS, contains, put};
use super::arena::{BitmapArena, Lingering};
use bitmap::WORDS;
use chunk_storage::{BucketKey, ChunkStorage, LayerCodec, LayerType};
use coordinates::{ChunkIndex, SuperchunkIndex};

impl BitmapArena {
    /// Makes `key`'s bitmap hot, decoding `layer` -- its encoded bitmap,
    /// `None` for one with no cell set: whether it was made hot now
    /// (`docs/bitplane_manager.md`, "Making hot, writing back, evicting").
    pub fn make_hot(&mut self, key: BucketKey, layer: Option<&[u64]>, codec: &mut LayerCodec) -> bool {
        assert_eq!(key.layer_type.bits(), 1, "one encoded bitmap makes a layer of a bit a cell hot");
        self.make_hot_with(key, layer.map(|layer| |bucket: &mut [u64]| codec.decode(layer, bucket.try_into().expect("a bitmap's words"))))
    }

    /// [`BitmapArena::make_hot`], the bucket's `cells` given, decoded
    /// elsewhere -- off the tick, say -- `None` for no cell set: a
    /// bitmap's words, times the layer's bits a cell
    /// ([`chunk_storage::wide`]).
    pub fn make_hot_cells(&mut self, key: BucketKey, cells: Option<&[u64]>) -> bool {
        self.make_hot_with(key, cells.map(|cells| |bucket: &mut [u64]| bucket.copy_from_slice(cells)))
    }

    /// Makes `key`'s bitmap hot, `fill` writing its cells into its
    /// bucket unless it is hot already or waiting in the ring, then
    /// counted: whether it was made hot now. With no `fill` it has no
    /// cell set, and no bucket.
    fn make_hot_with(&mut self, key: BucketKey, fill: Option<impl FnOnce(&mut [u64])>) -> bool {
        let (at, chunk) = (self.allocation(key.layer_type, key.chunk.superchunk()), key.chunk.place());
        let allocation = self.at_mut(at);
        if contains(allocation.flags.hot, chunk) {
            return false;
        }
        if !contains(allocation.flags.in_ring, chunk) {
            match fill {
                Some(fill) => {
                    let bits = key.layer_type.bits();
                    let bucket = allocation.keep(chunk);
                    fill(bucket);
                    // A count tile's cells: as many times its words as the layer has bits a cell.
                    let tile_words = COUNT_TILE_WORDS * bits as usize;
                    let tile_counts: [u16; COUNT_TILES_IN_CHUNK] = std::array::from_fn(|tile| chunk_storage::wide::cells_set(&bucket[tile * tile_words..][..tile_words], bits) as u16);
                    allocation.tile_counts[chunk] = tile_counts;
                    allocation.set_count(chunk, tile_counts.iter().map(|&count| count as u32).sum());
                }
                None => allocation.let_bucket_go(chunk),
            }
        }
        put(&mut allocation.flags.hot, chunk, true);
        allocation.hot_count += allocation.count(chunk);
        true
    }

    /// Makes hot the layers of `types` of the chunk at `chunk`, decoded
    /// from `storage`'s cold pool: only those are decoded, and the chunk's
    /// other layers stay encoded. A type the chunk has no layer of turns
    /// hot empty, and one already hot is left as it is: how many were made
    /// hot.
    pub fn make_hot_layers(&mut self, chunk: ChunkIndex, types: &[LayerType], storage: &ChunkStorage, codec: &mut LayerCodec) -> usize {
        let mut made = |arena: &mut Self, layer_type: LayerType| match layer_type.bits() {
            1 => arena.make_hot(BucketKey { layer_type, chunk }, storage.layer(chunk, layer_type), codec),
            // A wide layer: its planes decoded, a bit of every cell each.
            bits => {
                // With no plane kept, no cell holds a number: no bucket.
                let any = (0..bits).any(|bit| storage.layer(chunk, layer_type.plane(bit)).is_some());
                let fill = any.then_some(|bucket: &mut [u64]| {
                    bucket.fill(0);
                    let mut plane = [0; WORDS];
                    for bit in 0..bits {
                        if let Some(layer) = storage.layer(chunk, layer_type.plane(bit)) {
                            codec.decode(layer, &mut plane);
                            chunk_storage::wide::spread(&plane, bits, bit, bucket);
                        }
                    }
                });
                arena.make_hot_with(BucketKey { layer_type, chunk }, fill)
            }
        };
        types.iter().filter(|&&layer_type| made(self, layer_type)).count()
    }

    /// [`BitmapArena::make_hot_layers`], for every chunk of `superchunk`.
    pub fn make_hot_superchunk(&mut self, superchunk: SuperchunkIndex, types: &[LayerType], storage: &ChunkStorage, codec: &mut LayerCodec) -> usize {
        superchunk.chunks().map(|chunk| self.make_hot_layers(chunk, types, storage, codec)).sum()
    }

    /// Makes every bitmap over `superchunk` cold at once: its dirty
    /// buckets returned to be encoded, its allocations set aside,
    /// lingering (`docs/bitplane_manager.md`, "Lingering").
    pub fn make_cold_superchunk(&mut self, superchunk: SuperchunkIndex) -> Vec<(BucketKey, Box<[u64]>)> {
        let dirty = self.take_dirty(superchunk);
        if let Ok(entry) = self.lookup.superchunk(&self.directory, superchunk) {
            let lingering = Lingering { superchunk: self.directory.remove(entry), wanted: false };
            let at = self.lingering_at(superchunk).expect_err("hot, so not lingering");
            self.lingering.insert(at, lingering);
            self.lookup.forget();
            self.release_unused();
        }
        dirty
    }

    /// Where `superchunk` is among the lingering, or would go.
    pub(crate) fn lingering_at(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.lingering.binary_search_by_key(&superchunk, |lingering| lingering.superchunk.index)
    }

    /// Wants `superchunk` hot again: if it is lingering, it is kept, to be
    /// made hot as it is ([`BitmapArena::make_hot_again`]). Whether it is.
    pub fn hold(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(at) = self.lingering_at(superchunk) else {
            return false;
        };
        self.lingering[at].wanted = true;
        true
    }

    /// No longer wants `superchunk` hot again ([`BitmapArena::hold`]
    /// undone): lingering, it is let go once chunk storage holds its
    /// changes.
    pub fn let_go(&mut self, superchunk: SuperchunkIndex) {
        if let Ok(at) = self.lingering_at(superchunk) {
            self.lingering[at].wanted = false;
            self.release_unused();
        }
    }

    /// Makes `superchunk` hot again as it went cold, if it is lingering:
    /// whether it was.
    pub fn make_hot_again(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(at) = self.lingering_at(superchunk) else {
            return false;
        };
        let entry = self.lookup.superchunk(&self.directory, superchunk).expect_err("lingering, so not hot");
        self.directory.insert(entry, self.lingering.remove(at).superchunk);
        self.lookup.forget();
        true
    }

    /// How many superchunks are lingering.
    pub fn lingering(&self) -> usize {
        self.lingering.len()
    }
}
