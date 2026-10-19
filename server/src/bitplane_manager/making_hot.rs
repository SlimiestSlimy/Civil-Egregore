//! Bitmaps made hot and cold: a bucket, a chunk's layers, a superchunk
//! whole -- and a superchunk gone cold kept lingering.

use super::{COUNT_TILES_IN_CHUNK, COUNT_TILE_WORDS, contains, set_membership};
use super::arena::{BitmapArena, Lingering};
use crate::chunk_storage::{BucketKey, ChunkStorage, LayerCodec, LayerType};
use utilities::coordinates::{ChunkIndex, SuperchunkIndex};

impl BitmapArena {
    /// Makes `bucket_key`'s bitmap hot, decoding `layer` -- its encoded bitmap,
    /// `None` for one with no cell set: whether it was made hot now
    /// (`docs/bitplane_manager.md`, "Making hot, writing back, evicting").
    pub fn make_hot(&mut self, bucket_key: BucketKey, layer: Option<&[u64]>, layer_codec: &mut LayerCodec) -> bool {
        assert_eq!(bucket_key.layer_type.width_bits(), 1, "one encoded bitmap makes a layer of a bit a cell hot");
        self.make_hot_with(bucket_key, layer.map(|layer| |bucket: &mut [u64]| layer_codec.decode_cells(layer, bucket.try_into().expect("a bitmap's words"))))
    }

    /// [`BitmapArena::make_hot`], the bucket's `cells` given, decoded
    /// elsewhere -- off the tick, say -- `None` for no cell set: a
    /// bitmap's words, times the layer's bits a cell
    /// ([`crate::chunk_storage::wide`]).
    pub fn make_hot_cells(&mut self, bucket_key: BucketKey, cells: Option<&[u64]>) -> bool {
        self.make_hot_with(bucket_key, cells.map(|cells| |bucket: &mut [u64]| bucket.copy_from_slice(cells)))
    }

    /// Makes `bucket_key`'s bitmap hot, `fill_bucket` writing its cells into its
    /// bucket unless it is hot already or waiting in the ring, then
    /// counted: whether it was made hot now. With no `fill_bucket` it has no
    /// cell set, and no bucket.
    fn make_hot_with(&mut self, bucket_key: BucketKey, fill_bucket: Option<impl FnOnce(&mut [u64])>) -> bool {
        let (allocation_at, chunk) = (self.allocation(bucket_key.layer_type, bucket_key.chunk.superchunk()), bucket_key.chunk.place());
        let allocation = self.layer_at_mut(allocation_at);
        if contains(allocation.chunk_flags.hot, chunk) {
            return false;
        }
        if !contains(allocation.chunk_flags.in_ring, chunk) {
            match fill_bucket {
                Some(fill_given) => {
                    let bits_a_cell = bucket_key.layer_type.width_bits();
                    let bucket = allocation.bucket_or_insert(chunk);
                    fill_given(bucket);
                    // A count tile's cells: as many times its words as the layer has bits a cell -- over its tile's cells, of a plane of tiles.
                    let tile_words = (COUNT_TILE_WORDS * bits_a_cell as usize) >> bucket_key.layer_type.tile_shift();
                    assert!(tile_words > 0, "a plane of tiles has a word a count tile at the least");
                    let tile_counts: [u16; COUNT_TILES_IN_CHUNK] = std::array::from_fn(|tile| crate::chunk_storage::wide::cells_set(&bucket[tile * tile_words..][..tile_words], bits_a_cell) as u16);
                    allocation.tile_counts[chunk] = tile_counts;
                    allocation.set_count(chunk, tile_counts.iter().map(|&count| count as u32).sum());
                }
                None => allocation.let_bucket_go(chunk),
            }
        }
        set_membership(&mut allocation.chunk_flags.hot, chunk, true);
        allocation.hot_count += allocation.count(chunk);
        true
    }

    /// Makes hot the layers of `layer_types` of the chunk at `chunk`, decoded
    /// from `cold_storage`'s cold pool: only those are decoded, and the chunk's
    /// other layers stay encoded. A type the chunk has no layer of turns
    /// hot empty, and one already hot is left as it is: how many were made
    /// hot.
    pub fn make_hot_layers(&mut self, chunk: ChunkIndex, layer_types: &[LayerType], cold_storage: &ChunkStorage, layer_codec: &mut LayerCodec) -> usize {
        let mut make_layer_hot = |bitmap_arena: &mut Self, layer_type: LayerType| {
            if layer_type.is_a_bitplane() {
                return bitmap_arena.make_hot(BucketKey { layer_type, chunk }, cold_storage.layer(chunk, layer_type), layer_codec);
            }
            // A wide layer, or one of tiles: its bits decoded in order. With none kept, no cell holds a number: no bucket.
            let fill_bucket = cold_storage.layer(chunk, layer_type).map(|layer| |bucket: &mut [u64]| crate::chunk_storage::wide::decode_bucket(layer_type, layer, layer_codec, bucket));
            bitmap_arena.make_hot_with(BucketKey { layer_type, chunk }, fill_bucket)
        };
        layer_types.iter().filter(|&&layer_type| make_layer_hot(self, layer_type)).count()
    }

    /// Makes every bitmap over `superchunk` cold at once: its dirty
    /// buckets returned to be encoded, its allocations set aside,
    /// lingering (`docs/bitplane_manager.md`, "Lingering").
    pub fn make_cold_superchunk(&mut self, superchunk: SuperchunkIndex) -> Vec<(BucketKey, Box<[u64]>)> {
        let dirty = self.take_dirty(superchunk);
        if let Ok(entry_index) = self.lookup.superchunk(&self.directory, superchunk) {
            let lingering = Lingering { superchunk: self.directory.remove(entry_index), wanted_hot_again: false };
            let lingering_index = self.lingering_at(superchunk).expect_err("hot, so not lingering");
            self.lingering.insert(lingering_index, lingering);
            self.lookup.forget_last();
            self.release_unused();
        }
        dirty
    }

    /// Where `superchunk` is among the lingering, or would go.
    pub(crate) fn lingering_at(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.lingering.binary_search_by_key(&superchunk, |lingering| lingering.superchunk.superchunk_index)
    }

    /// Wants `superchunk` hot again: if it is lingering, it is kept, to be
    /// made hot as it is ([`BitmapArena::make_hot_again`]). Whether it is.
    pub fn hold_lingering(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(lingering_index) = self.lingering_at(superchunk) else {
            return false;
        };
        self.lingering[lingering_index].wanted_hot_again = true;
        true
    }

    /// Makes `superchunk` hot again as it went cold, if it is lingering:
    /// whether it was.
    pub fn make_hot_again(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(lingering_index) = self.lingering_at(superchunk) else {
            return false;
        };
        let entry_index = self.lookup.superchunk(&self.directory, superchunk).expect_err("lingering, so not hot");
        self.directory.insert(entry_index, self.lingering.remove(lingering_index).superchunk);
        self.lookup.forget_last();
        true
    }

    /// Whether `superchunk` is lingering: cold, its buckets kept until
    /// chunk storage holds its changes.
    pub fn is_lingering(&self, superchunk: SuperchunkIndex) -> bool {
        self.lingering_at(superchunk).is_ok()
    }

    /// How many superchunks are lingering.
    pub fn lingering(&self) -> usize {
        self.lingering.len()
    }
}
