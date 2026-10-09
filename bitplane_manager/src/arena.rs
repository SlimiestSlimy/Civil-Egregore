//! The arena: the directory of hot superchunks, the lingering ones, and
//! what is read off them.

use super::{COUNT_TILES_IN_CHUNK, NotHot, contains, members};
use super::superchunk_layer::{ChunkFlags, SuperchunkLayer};
use super::reader::{LayerAt, Lookup};
use super::superchunk::{Bucket, Superchunk};
use super::writes::WriteQueues;
use chunk_storage::{BucketKey, LayerType, Wide, Width};
use coordinates::{CHUNKS_IN_SUPERCHUNK, CellIndex, ChunkIndex, SuperchunkIndex};

/// A superchunk gone cold as a whole ([`BitmapArena::make_cold_superchunk`]):
/// its allocations kept as they were -- hot buckets and all, counted --
/// until chunk storage holds its changes, or for as long as it is
/// wanted hot again, when it is made hot as it is, nothing decoded
/// ([`BitmapArena::make_hot_again`]).
pub(crate) struct Lingering {
    /// The superchunk, as it was when it went cold.
    pub(crate) superchunk: Superchunk,
    /// Whether it is wanted hot again: kept until it is.
    pub(crate) wanted: bool,
}

impl Lingering {
    /// Whether it can be let go: unwanted, and every change of it in
    /// chunk storage's cold pool -- none on its way, none in the ring.
    pub(crate) fn done(&self) -> bool {
        !self.wanted && self.superchunk.on_their_way == 0 && self.superchunk.layers.iter().all(|layer| layer.flags.in_ring == 0)
    }
}

/// The hot bitmaps, in allocations a superchunk each.
pub struct BitmapArena {
    /// Every superchunk with a layer in use, sorted by superchunk index:
    /// the hot ones.
    pub(crate) directory: Vec<Superchunk>,
    /// The superchunks gone cold whose allocations are kept, sorted by
    /// superchunk index.
    pub(crate) lingering: Vec<Lingering>,
    /// The arena's own lookups, remembering the last.
    pub(crate) lookup: Lookup,
    /// Writes queued from outside a tick, not yet applied.
    pub(crate) queued: WriteQueues,
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
        Self { directory: Vec::new(), lingering: Vec::new(), lookup: Lookup::default(), queued: WriteQueues::default() }
    }

    /// Every allocation in use, superchunk by superchunk in Morton order,
    /// in each by type.
    pub(crate) fn layers(&self) -> impl Iterator<Item = &SuperchunkLayer> {
        self.directory.iter().flat_map(|entry| &entry.layers)
    }

    /// How many bitmaps are hot.
    pub fn len(&self) -> usize {
        self.layers().map(|layer| layer.flags.hot.count_ones() as usize).sum()
    }

    /// Whether no bitmap is hot.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many allocations are in use: holding a hot bitmap, or one
    /// waiting in the ring.
    pub fn allocations(&self) -> usize {
        self.layers().count()
    }

    /// The allocation at `at`.
    fn at(&self, (entry, layer): LayerAt) -> &SuperchunkLayer {
        &self.directory[entry].layers[layer]
    }

    /// The allocation at `at`, to change.
    pub(crate) fn at_mut(&mut self, (entry, layer): LayerAt) -> &mut SuperchunkLayer {
        &mut self.directory[entry].layers[layer]
    }

    /// Where `key`'s bucket is, if it is hot: its allocation, and the
    /// bucket's index there.
    pub(crate) fn hot(&self, key: BucketKey) -> Option<(LayerAt, usize)> {
        let at = self.lookup.find(&self.directory, key.layer_type, key.chunk.superchunk())?;
        contains(self.at(at).flags.hot, key.chunk.place()).then_some((at, key.chunk.place()))
    }

    /// Whether `key`'s bitmap is hot.
    pub fn is_hot(&self, key: BucketKey) -> bool {
        self.hot(key).is_some()
    }

    /// The allocation for `layer_type` over `superchunk`: the one in
    /// use, or else a new one, with no bucket yet.
    pub(crate) fn allocation(&mut self, layer_type: LayerType, superchunk: SuperchunkIndex) -> LayerAt {
        if let Some(at) = self.lookup.find(&self.directory, layer_type, superchunk) {
            return at;
        }
        self.lookup.forget();
        let entry = self.lookup.superchunk(&self.directory, superchunk).unwrap_or_else(|entry| {
            self.directory.insert(entry, Superchunk { index: superchunk, layers: Vec::new(), on_their_way: 0 });
            entry
        });
        let layers = &mut self.directory[entry].layers;
        let layer = layers.binary_search_by_key(&layer_type, |layer| layer.layer_type).expect_err("not in use");
        let new = SuperchunkLayer { layer_type, buckets: Vec::new(), kept: 0, flags: ChunkFlags::default(), counts_less_one: [0; CHUNKS_IN_SUPERCHUNK], hot_count: 0, tile_counts: [[0; COUNT_TILES_IN_CHUNK]; CHUNKS_IN_SUPERCHUNK] };
        layers.insert(layer, new);
        (entry, layer)
    }
}

impl BitmapArena {
    /// How many cells of `layer_type` are set over `superchunk`, in its
    /// hot bitmaps: what weighs the superchunk when sampling.
    pub fn superchunk_count(&self, layer_type: LayerType, superchunk: SuperchunkIndex) -> u32 {
        self.lookup.find(&self.directory, layer_type, superchunk).map_or(0, |at| self.at(at).hot_count)
    }

    /// `key`'s bitmap, to read, if it is hot.
    pub fn bucket(&self, key: BucketKey) -> Option<Bucket<'_>> {
        self.hot(key).map(|(at, chunk)| {
            let allocation = self.at(at);
            Bucket { words: allocation.words(chunk), count: allocation.count(chunk) }
        })
    }

    /// Whether `layer_type` holds at `cell`, anywhere in the world: its
    /// superchunk, chunk and bit taken from its index.
    pub fn holds(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.lookup.holds(&self.directory, layer_type, cell)
    }

    /// The number `plane` holds at `cell`, anywhere in the world.
    pub fn value<W: Width>(&self, plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.lookup.value(&self.directory, plane, cell)
    }

    /// Every allocation of `layer_type`, with its superchunk, superchunk
    /// by superchunk in Morton order.
    fn layers_of(&self, layer_type: LayerType) -> impl Iterator<Item = (SuperchunkIndex, &SuperchunkLayer)> {
        self.directory.iter().filter_map(move |entry| entry.layer_index(layer_type).map(|layer| (entry.index, &entry.layers[layer])))
    }

    /// Every superchunk with an allocation in use, sorted by Morton
    /// index: to read, from as many threads as like.
    pub fn superchunks(&self) -> &[Superchunk] {
        &self.directory
    }

    /// Every superchunk with an allocation in use, sorted.
    pub fn superchunk_indices(&self) -> Vec<SuperchunkIndex> {
        self.directory.iter().map(Superchunk::index).collect()
    }

    /// Every superchunk with an allocation in use, sorted by Morton
    /// index: to change, each apart from the others. Superchunks are
    /// neither added nor removed through it.
    pub fn superchunks_mut(&mut self) -> &mut [Superchunk] {
        &mut self.directory
    }

    /// Every hot bitmap of `layer_type`: superchunk by superchunk in their
    /// Morton order, and in each the chunks in theirs.
    pub fn run(&self, layer_type: LayerType) -> impl Iterator<Item = (ChunkIndex, Bucket<'_>)> {
        self.layers_of(layer_type).flat_map(move |(superchunk, allocation)| {
            members(allocation.flags.hot)
                .map(move |chunk| (ChunkIndex::of(superchunk, chunk), Bucket { words: allocation.words(chunk), count: allocation.count(chunk) }))
        })
    }

    /// Every hot bitmap's key, in the arena's order: by superchunk, then
    /// type, then chunk, superchunks and chunks in Morton order.
    pub fn keys(&self) -> impl Iterator<Item = BucketKey> + '_ {
        self.directory.iter().flat_map(move |entry| {
            entry.layers.iter().flat_map(move |allocation| {
                members(allocation.flags.hot).map(move |chunk| BucketKey { layer_type: allocation.layer_type, chunk: ChunkIndex::of(entry.index, chunk) })
            })
        })
    }
}
