//! Bitmaps made hot and cold: a bucket, a chunk's layers, a superchunk
//! whole -- and a superchunk gone cold kept cooled.

use super::arena::{BitmapArena, Cooled};
use super::superchunk::Superchunk;
use crate::chunk_storage::{BucketKey, ChunkStorage, LayerCodec, LayerType};
use utilities::coordinates::{ChunkIndex, SuperchunkIndex};

impl BitmapArena {
    /// Makes `bucket_key`'s bitmap hot, decoding `layer` -- its encoded bitmap,
    /// `None` for one with no cell set: whether it was made hot now
    /// (`docs/bitplane_manager.md`, "Making hot, writing back, evicting").
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn make_hot(&mut self, bucket_key: BucketKey, layer: Option<&[u64]>, layer_codec: &mut LayerCodec) -> bool {
        assert_eq!(bucket_key.layer_type.width_bits(), 1, "one encoded bitmap makes a layer of a bit a cell hot");
        self.make_hot_with(bucket_key, layer.map(|layer| |bucket: &mut [u64]| layer_codec.decode_cells(layer, bucket.try_into().expect("a bitmap's words"))))
    }

    /// [`BitmapArena::make_hot`], the bucket's `cells` given, decoded
    /// elsewhere -- off the tick, say -- `None` for no cell set: a
    /// bitmap's words, times the layer's bits a cell
    /// ([`crate::chunk_storage::wide`]).
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn make_hot_cells(&mut self, bucket_key: BucketKey, cells: Option<&[u64]>) -> bool {
        self.make_hot_with(bucket_key, cells.map(|cells| |bucket: &mut [u64]| bucket.copy_from_slice(cells)))
    }

    /// Makes `bucket_key`'s bitmap hot, `fill_bucket` writing its cells into its
    /// bucket unless it is hot already or waiting in the ring, then
    /// counted: whether it was made hot now. With no `fill_bucket` it has no
    /// cell set, and no bucket.
    fn make_hot_with(&mut self, bucket_key: BucketKey, fill_bucket: Option<impl FnOnce(&mut [u64])>) -> bool {
        let allocation_at = self.allocation(bucket_key.layer_type, bucket_key.chunk.superchunk());
        self.layer_at_mut(allocation_at).make_chunk_hot(bucket_key.chunk.place(), fill_bucket)
    }

    /// Makes hot the layers of `layer_types` of the chunk at `chunk`, decoded
    /// from `cold_storage`'s cold pool: only those are decoded, and the chunk's
    /// other layers stay encoded. A type the chunk has no layer of turns
    /// hot empty, and one already hot is left as it is: how many were made
    /// hot.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
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
    /// cooled (`docs/bitplane_manager.md`, "Lingering").
    pub fn make_cold_superchunk(&mut self, superchunk: SuperchunkIndex) -> Vec<(BucketKey, Box<[u64]>)> {
        let dirty = self.take_dirty(superchunk);
        if let Ok(entry_index) = self.lookup.superchunk(&self.directory, superchunk) {
            let cooled = Cooled { superchunk: self.directory.remove(entry_index), wanted_hot_again: false };
            let cooled_index = self.cooled_at(superchunk).expect_err("hot, so not cooled");
            self.cooled.insert(cooled_index, cooled);
            self.lookup.forget_last();
            self.release_unused();
        }
        dirty
    }

    /// Where `superchunk` is among the cooled, or would go.
    pub(crate) fn cooled_at(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.cooled.binary_search_by_key(&superchunk, |cooled| cooled.superchunk.superchunk_index)
    }

    /// Wants `superchunk` hot again: if it is cooled, it is kept, to be
    /// made hot as it is ([`BitmapArena::make_hot_again`]). Whether it is.
    pub fn hold_cooled(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(cooled_index) = self.cooled_at(superchunk) else {
            return false;
        };
        self.cooled[cooled_index].wanted_hot_again = true;
        true
    }

    /// Makes `superchunk` hot again as it went cold, if it is cooled:
    /// whether it was.
    pub fn make_hot_again(&mut self, superchunk: SuperchunkIndex) -> bool {
        let Ok(cooled_index) = self.cooled_at(superchunk) else {
            return false;
        };
        let entry_index = self.lookup.superchunk(&self.directory, superchunk).expect_err("cooled, so not hot");
        self.directory.insert(entry_index, self.cooled.remove(cooled_index).superchunk);
        self.lookup.forget_last();
        true
    }

    /// Makes `warmed` hot, whole, as it was warmed apart from the arena:
    /// from its image ([`Superchunk::from_image`]), no bucket dirty, or
    /// generated ([`Superchunk::to_generate`]), every bucket a write made
    /// dirty, to be written back as it cools.
    pub fn make_hot_warmed(&mut self, warmed: Superchunk) {
        let entry_index = self.lookup.superchunk(&self.directory, warmed.superchunk_index).expect_err("warmed, so not hot");
        self.directory.insert(entry_index, warmed);
        self.lookup.forget_last();
    }

    /// Whether `superchunk` is cooled: cold, its buckets kept until
    /// chunk storage holds its changes.
    pub fn is_cooled(&self, superchunk: SuperchunkIndex) -> bool {
        self.cooled_at(superchunk).is_ok()
    }

    /// How many superchunks are cooled.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn cooled_count(&self) -> usize {
        self.cooled.len()
    }
}
