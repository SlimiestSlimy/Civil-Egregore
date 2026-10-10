//! A superchunk's layers as the simulation is handed them: the
//! superchunk itself, a layer of it to read, and a hot bucket.

use super::{COUNT_TILES_IN_CHUNK, contains};
use super::superchunk_layer::{SuperchunkLayer};
use super::writes::{Write, WritesApplied, apply_in};
use bitmap::{BITS_PER_WORD, CellWords};
use chunk_storage::LayerType;
use coordinates::{CellIndex, SuperchunkIndex};

/// One hot bitmap, to read.
pub struct Bucket<'a> {
    /// Its cells' words: a bitmap's, times its layer's bits a cell.
    pub(crate) words: &'a [u64],
    /// How many of them are set.
    pub(crate) count: u32,
}

impl Bucket<'_> {
    /// How many cells are set.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Whether the cell at `place` in the chunk is set.
    pub fn get(&self, place: usize) -> bool {
        self.cells()[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & 1 == 1
    }

    /// Every cell, in Morton order, 64 a word: of a layer a bit a cell.
    pub fn cells(&self) -> &CellWords {
        self.words.try_into().expect("a bucket of a bit a cell is a bitmap's words")
    }

    /// Every cell, in Morton order, each as many bits as its layer is
    /// wide, the first lowest in the first word.
    pub fn words(&self) -> &[u64] {
        self.words
    }
}

/// One superchunk of the arena: its allocations, sorted by layer type.
/// It owns them, blocks and all, so superchunks are
/// changed apart -- on different threads, say: a simulation reads any
/// through [`Superchunk::layer`] or a [`Reader`], and changes one only
/// through [`Superchunk::apply`].
pub struct Superchunk {
    /// Which superchunk it is.
    pub(crate) index: SuperchunkIndex,
    /// Its allocations, sorted by layer type, one a type.
    pub(crate) layers: Vec<SuperchunkLayer>,
    /// Its write-backs taken ([`BitmapArena::take_dirty`]) and not yet
    /// in the ring ([`BitmapArena::written_back`]): while any is on its
    /// way, its buckets are newer than chunk storage.
    pub(crate) on_their_way: u32,
}

impl Superchunk {
    /// Where `layer_type` is among the layers, if it has one.
    pub(crate) fn layer_index(&self, layer_type: LayerType) -> Option<usize> {
        self.layers.binary_search_by_key(&layer_type, |layer| layer.layer_type).ok()
    }

    /// Which superchunk it is.
    pub fn index(&self) -> SuperchunkIndex {
        self.index
    }

    /// Its layer of `layer_type`, to read, if it has one in use.
    pub fn layer(&self, layer_type: LayerType) -> Option<LayerView<'_>> {
        self.layer_index(layer_type).map(|layer| LayerView(&self.layers[layer]))
    }

    /// The number `layer_type` holds at `at`, a cell of this superchunk,
    /// as it is now -- 1 or 0 on a layer of a bit a cell: none where
    /// its bitmap is not hot. What a write that says what its rule saw
    /// is held against (`docs/bitplane_manager.md`, "Writes").
    pub fn value_at(&self, layer_type: LayerType, at: CellIndex) -> Option<u32> {
        debug_assert_eq!(at.superchunk(), self.index, "a cell of another superchunk");
        let layer = &self.layers[self.layer_index(layer_type)?];
        let chunk = at.chunk().place();
        contains(layer.flags.hot, chunk).then(|| layer.value(chunk, at.place()))
    }

    /// Applies the part of `write`, to `layer_type`'s bitplane, that
    /// lies in this superchunk: cells in bitmaps not hot counted missed.
    pub fn apply(&mut self, layer_type: LayerType, write: Write, applied: &mut WritesApplied) {
        apply_in(Some(&mut self.layers), self.index, layer_type, write, applied);
    }
}

/// One superchunk's layer of one type, to read: its hot buckets, their
/// counts and their cells -- what sampling finds its cells by.
#[derive(Clone, Copy)]
pub struct LayerView<'a>(&'a SuperchunkLayer);

impl LayerView<'_> {
    /// How many cells the hot buckets have set, together.
    pub fn hot_count(&self) -> u32 {
        self.0.hot_count
    }

    /// Whether the bucket of the chunk at `chunk`, in Morton order, is
    /// hot.
    pub fn is_hot(&self, chunk: usize) -> bool {
        contains(self.0.flags.hot, chunk)
    }

    /// How many cells the bucket of the chunk at `chunk` has set.
    pub fn count(&self, chunk: usize) -> u32 {
        self.0.count(chunk)
    }

    /// The cells of the chunk at `chunk`, in Morton order, 64 a word.
    pub fn cells(&self, chunk: usize) -> &CellWords {
        self.0.cells(chunk)
    }

    /// How many cells each count tile of the bucket of the chunk at
    /// `chunk` has set: [`COUNT_TILE_WORDS`] words a count tile, in
    /// Morton order.
    pub fn tile_counts(&self, chunk: usize) -> &[u16; COUNT_TILES_IN_CHUNK] {
        &self.0.tile_counts[chunk]
    }
}
