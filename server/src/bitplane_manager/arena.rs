//! The arena: the directory of hot superchunks, the cooled ones, and
//! what is read off them.

use super::{NotHot, contains, members};
use super::superchunk_layer::SuperchunkLayer;
use super::reader::{LayerAt, Lookup};
use super::superchunk::{Bucket, Superchunk};
use super::writes::WriteQueues;
use crate::chunk_storage::{BucketKey, LayerType};
use crate::bitplane_manager::{WidePlane, Width};
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex};

/// A superchunk gone cold as a whole ([`BitmapArena::make_cold_superchunk`]):
/// its allocations kept as they were -- hot buckets and all, counted --
/// until chunk storage holds its changes, or for as long as it is
/// wanted hot again, when it is made hot as it is, nothing decoded
/// ([`BitmapArena::make_hot_again`]).
pub(crate) struct Cooled {
    /// The superchunk, as it was when it went cold.
    pub(crate) superchunk: Superchunk,
    /// Whether it is wanted hot again: kept until it is.
    pub(crate) wanted_hot_again: bool,
}

impl Cooled {
    /// Whether it can be let go: unwanted, and every change of it in
    /// chunk storage's cold pool -- none on its way, none in the ring.
    pub(crate) fn can_be_let_go(&self) -> bool {
        !self.wanted_hot_again && self.superchunk.on_their_way == 0 && self.superchunk.layers.iter().all(|layer| layer.chunk_flags.in_ring == 0)
    }
}

/// The hot bitmaps, in allocations a superchunk each.
pub struct BitmapArena {
    /// Every superchunk with a layer in use, sorted by superchunk index:
    /// the hot ones.
    pub(crate) directory: Vec<Superchunk>,
    /// The superchunks gone cold whose allocations are kept, sorted by
    /// superchunk index.
    pub(crate) cooled: Vec<Cooled>,
    /// The arena's own lookups, remembering the last.
    pub(crate) lookup: Lookup,
    /// Writes queued from outside a tick, not yet applied.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) queued_writes: WriteQueues,
}

impl Default for BitmapArena {
    /// The same as [`BitmapArena::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapArena {
    /// An arena with no bitmap hot.
    pub fn new() -> Self {
        Self { directory: Vec::new(), cooled: Vec::new(), lookup: Lookup::default(), queued_writes: WriteQueues::default() }
    }

    /// Every allocation in use, superchunk by superchunk in Morton order,
    /// in each by type.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) fn layers(&self) -> impl Iterator<Item = &SuperchunkLayer> {
        self.directory.iter().flat_map(|directory_entry| &directory_entry.layers)
    }

    /// How many bitmaps are hot.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn len(&self) -> usize {
        self.layers().map(|layer| layer.chunk_flags.hot.count_ones() as usize).sum()
    }

    /// Whether no bitmap is hot.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many allocations are in use: holding a hot bitmap, or one
    /// waiting in the ring.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn allocations(&self) -> usize {
        self.layers().count()
    }

    /// The allocation at `at`.
    fn layer_at(&self, (entry_index, layer): LayerAt) -> &SuperchunkLayer {
        &self.directory[entry_index].layers[layer]
    }

    /// The allocation at `at`, to change.
    pub(crate) fn layer_at_mut(&mut self, (entry_index, layer): LayerAt) -> &mut SuperchunkLayer {
        &mut self.directory[entry_index].layers[layer]
    }

    /// Where `bucket_key`'s bucket is, if it is hot: its allocation, and the
    /// bucket's index there.
    pub(crate) fn hot(&self, bucket_key: BucketKey) -> Option<(LayerAt, usize)> {
        let allocation_at = self.lookup.find_allocation(&self.directory, bucket_key.layer_type, bucket_key.chunk.superchunk())?;
        contains(self.layer_at(allocation_at).chunk_flags.hot, bucket_key.chunk.place()).then_some((allocation_at, bucket_key.chunk.place()))
    }

    /// Whether `bucket_key`'s bitmap is hot.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn is_hot(&self, bucket_key: BucketKey) -> bool {
        self.hot(bucket_key).is_some()
    }

    /// The allocation for `layer_type` over `superchunk`: the one in
    /// use, or else a new one, with no bucket yet.
    pub(crate) fn allocation(&mut self, layer_type: LayerType, superchunk: SuperchunkIndex) -> LayerAt {
        if let Some(allocation_at) = self.lookup.find_allocation(&self.directory, layer_type, superchunk) {
            return allocation_at;
        }
        self.lookup.forget_last();
        let entry_index = self.lookup.superchunk(&self.directory, superchunk).unwrap_or_else(|vacant_entry_index| {
            self.directory.insert(vacant_entry_index, Superchunk { superchunk_index: superchunk, layers: Vec::new(), on_their_way: 0 });
            vacant_entry_index
        });
        let layers = &mut self.directory[entry_index].layers;
        let layer = layers.binary_search_by_key(&layer_type, |layer| layer.layer_type).expect_err("not in use");
        layers.insert(layer, SuperchunkLayer::empty(layer_type, 0));
        (entry_index, layer)
    }
}

impl BitmapArena {
    /// How many cells of `layer_type` are set over `superchunk`, in its
    /// hot bitmaps: what weighs the superchunk when sampling.
    pub fn superchunk_count(&self, layer_type: LayerType, superchunk: SuperchunkIndex) -> u32 {
        self.lookup.find_allocation(&self.directory, layer_type, superchunk).map_or(0, |allocation_at| self.layer_at(allocation_at).hot_count)
    }

    /// `bucket_key`'s bitmap, to read, if it is hot.
    pub fn bucket(&self, bucket_key: BucketKey) -> Option<Bucket<'_>> {
        self.hot(bucket_key).map(|(allocation_at, chunk)| {
            let allocation = self.layer_at(allocation_at);
            Bucket { words: allocation.words(chunk), count: allocation.count(chunk) }
        })
    }

    /// Whether `layer_type` holds at `cell`, anywhere in the world: its
    /// superchunk, chunk and bit taken from its index.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn layer_holds_cell(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.lookup.layer_holds_cell(&self.directory, layer_type, cell)
    }

    /// The number `wide_plane` holds at `cell`, anywhere in the world.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn value<W: Width>(&self, wide_plane: WidePlane<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.lookup.value(&self.directory, wide_plane, cell)
    }

    /// Every allocation of `layer_type`, with its superchunk, superchunk
    /// by superchunk in Morton order.
    fn layers_of(&self, layer_type: LayerType) -> impl Iterator<Item = (SuperchunkIndex, &SuperchunkLayer)> {
        self.directory.iter().filter_map(move |directory_entry| directory_entry.layer_index(layer_type).map(|layer| (directory_entry.superchunk_index, &directory_entry.layers[layer])))
    }

    /// Every superchunk with an allocation in use, sorted by Morton
    /// index: to read, from as many threads as like.
    pub fn superchunks(&self) -> &[Superchunk] {
        &self.directory
    }

    /// Every superchunk with an allocation in use, sorted.
    pub fn superchunk_indices(&self) -> Vec<SuperchunkIndex> {
        self.directory.iter().map(Superchunk::superchunk_index).collect()
    }

    /// Every superchunk with an allocation in use, sorted by Morton
    /// index: to change, each apart from the others. Superchunks are
    /// neither added nor removed through it.
    pub fn superchunks_mut(&mut self) -> &mut [Superchunk] {
        &mut self.directory
    }

    /// Every hot bitmap of `layer_type`: superchunk by superchunk in their
    /// Morton order, and in each the chunks in theirs.
    pub fn buckets_of(&self, layer_type: LayerType) -> impl Iterator<Item = (ChunkIndex, Bucket<'_>)> {
        self.layers_of(layer_type).flat_map(move |(superchunk, allocation)| {
            members(allocation.chunk_flags.hot)
                .map(move |chunk| (ChunkIndex::from_superchunk_place(superchunk, chunk), Bucket { words: allocation.words(chunk), count: allocation.count(chunk) }))
        })
    }

    /// Every hot bitmap's key, in the arena's order: by superchunk, then
    /// type, then chunk, superchunks and chunks in Morton order.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn hot_bucket_keys(&self) -> impl Iterator<Item = BucketKey> + '_ {
        self.directory.iter().flat_map(move |directory_entry| {
            directory_entry.layers.iter().flat_map(move |allocation| {
                members(allocation.chunk_flags.hot).map(move |chunk| BucketKey { layer_type: allocation.layer_type, chunk: ChunkIndex::from_superchunk_place(directory_entry.superchunk_index, chunk) })
            })
        })
    }
}
