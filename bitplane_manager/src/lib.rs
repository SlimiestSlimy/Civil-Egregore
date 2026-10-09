//! Civil Egregore's bitplane manager: the bitmap arena, the hot bitmaps, raw,
//! one a bucket -- the layers whose cells are being read or changed,
//! decoded from chunk storage's cold pool and nothing more. It is where
//! cells are read and changed: chunk storage holds whole encoded layers
//! only.
//!
//! The arena is made of allocations, each holding one layer type over
//! one superchunk: an array of buckets, one for each of its chunks that
//! has a cell set -- 16 at the most -- in the chunks' Morton order
//! ([`ChunkIndex::place`]). A chunk with no cell set has no bucket: its
//! cells are read as clear, and the first cell set in it makes it one,
//! put in its place among the others. A chunk's bucket is found by how
//! many chunks before it have one -- a count of bits, no search.
//!
//! Which allocation holds which layer type over which superchunk is a
//! small directory: the superchunks in use, sorted by superchunk index,
//! and for each its allocations, sorted by layer type -- the one thing
//! ever sorted, and it holds no bitmaps. The last superchunk looked up
//! is remembered, so the runs of lookups in one superchunk that
//! Morton-ordered work makes search only its few types. The allocations lie
//! wherever they were made; each is one run of memory in Morton order.
//!
//! The arena grows an allocation at a time, as a layer type turns hot
//! over a superchunk it had none of, and an allocation a bucket at a
//! time. An allocation none of whose chunks is hot or waiting in the
//! ring (below) leaves the directory, its buckets with it.
//!
//! Cells are changed by writes, batched (`writes`): queued, then applied
//! in order. Each superchunk ([`Superchunk`]) owns its blocks, so
//! superchunks are read and changed apart: the simulation
//! (`../simulation`) samples and reads them on as many threads as it
//! likes ([`LayerView`], [`Reader`]), and applies writes to each
//! ([`Superchunk::apply`]).
//!
//! A bucket changed since it was decoded is dirty, and is written back
//! into chunk storage's writeback ring (`../chunk_storage`) encoded --
//! no words where no cell is set, since a type with no cell set has no
//! layer -- in two halves, so encoding can be off the tick: taken
//! ([`BitmapArena::take_dirty`]), then put in the ring
//! ([`BitmapArena::written_back`]). A dirty bucket must be written back
//! before it is evicted. The ring is never read to make a bitmap hot,
//! so a bucket written back stays in its allocation, evicted or not,
//! until chunk storage flushes its superchunk into the cold pool: a
//! bitmap evicted and made hot again before then is the bucket as it
//! was, not decoded.
//!
//! A superchunk made cold as a whole ([`BitmapArena::make_cold_superchunk`])
//! leaves the directory at once, nothing encoded or flushed: its
//! allocations are set aside, lingering, until chunk storage holds its
//! changes -- made hot again as they are if wanted before then.

//! The design: `docs/bitplane_manager.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
pub mod transient_data;
mod writes;

pub use writes::{count_missed, WritesApplied, Shape, Write, WriteOp, WriteQueues};

use bitmap::window::{in_word_tile, left_columns, rows_from_morton, top_rows, window, PLACE_IN_WORD_TILE, WORD_TILE_SIDE};
use bitmap::{CellWords, BITS_PER_WORD, WORDS};
use utilities::cache::prefetch;
use chunk_storage::{ChunkStorage, LayerCodec, LayerType, Wide, Width};
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use std::cell::Cell;
use writes::apply_in;

pub use chunk_storage::BucketKey;

/// A cell was asked of a bitmap that is not hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotHot(pub BucketKey);

/// A set of a superchunk's chunks, one bit each, by place.
type ChunkSet = u16;

const _: () = assert!(ChunkSet::BITS as usize == CHUNKS_IN_SUPERCHUNK, "a chunk set holds a bit for every chunk");

/// Whether `set` holds the chunk at `index`.
fn contains(set: ChunkSet, index: usize) -> bool {
    set >> index & 1 == 1
}

/// Puts the chunk at `index` in `set`, or takes it out.
fn put(set: &mut ChunkSet, index: usize, member: bool) {
    if member {
        *set |= 1 << index;
    } else {
        *set &= !(1 << index);
    }
}

/// The chunks in `set`, in Morton order.
fn members(set: ChunkSet) -> impl Iterator<Item = usize> {
    (0..CHUNKS_IN_SUPERCHUNK).filter(move |&index| contains(set, index))
}

/// The most bits a cell a layer has.
const WIDEST: usize = 16;

/// A bucket with no cell set, however wide: what a chunk with no bucket
/// is read as.
static NO_CELLS: [u64; WORDS * WIDEST] = [0; WORDS * WIDEST];

/// Words in a count tile: the 32x32 cells whose set cells are counted,
/// so sampling and the far search pass over an empty one whole. Two
/// lines of memory, so a cell is found in its count tile in a short walk.
pub const COUNT_TILE_WORDS: usize = 16;
/// Count tiles in a bitmap.
pub const COUNT_TILES_IN_CHUNK: usize = WORDS / COUNT_TILE_WORDS;
/// Cells in a count tile.
const COUNT_TILE_CELLS: usize = COUNT_TILE_WORDS * BITS_PER_WORD;

/// The coarsest scale [`Reader::any_in_tile`] is asked of: tiles `2^6`
/// cells a side, four count tiles each.
pub const COARSEST_SCALE: u32 = 6;
/// Tiles of the coarsest scale in a chunk.
pub const COARSEST_TILES_IN_CHUNK: usize = 16;
/// Count tiles in a tile of the coarsest scale.
const COUNTS_IN_COARSEST: usize = COUNT_TILES_IN_CHUNK / COARSEST_TILES_IN_CHUNK;
const _: () = assert!(COUNT_TILE_CELLS * COUNTS_IN_COARSEST == 1 << (2 * COARSEST_SCALE));

/// A superchunk layer's four chunk sets, a bit a chunk each, packed
/// together in 8 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ChunkFlags {
    /// The chunks whose buckets are hot.
    hot: ChunkSet,
    /// The hot chunks changed since they were decoded.
    dirty: ChunkSet,
    /// The chunks written back to the ring and not yet flushed: their
    /// buckets hold their newest cells, hot or not.
    in_ring: ChunkSet,
    /// The chunks whose buckets have a cell set.
    nonempty: ChunkSet,
}

const _: () = assert!(size_of::<ChunkFlags>() == 4 * size_of::<ChunkSet>(), "the four chunk sets packed together");

/// An allocation: one layer type over one superchunk. It holds a bucket
/// for each chunk that has needed one, in Morton order -- a chunk with
/// none has no cell set -- beside which chunks are hot, dirty and
/// waiting, and the counts. It owns its buckets, so a superchunk's
/// allocations are changed apart from every other's.
struct SuperchunkLayer {
    /// The layer's type.
    layer_type: LayerType,
    /// The buckets of the chunks in `kept`, one after another in Morton
    /// order: [`SuperchunkLayer::bucket_words`] words each.
    buckets: Vec<u64>,
    /// The chunks that have a bucket.
    kept: ChunkSet,
    /// Which chunks are hot, dirty, waiting in the ring and non-empty.
    flags: ChunkFlags,
    /// How many cells each bucket has set, less one, by place --
    /// a bucket with any cell set has 1 to 65,536 of them, so a `u16`
    /// holds the count -- meaningful where the bucket is non-empty
    /// and hot or waiting in the ring.
    counts_less_one: [u16; CHUNKS_IN_SUPERCHUNK],
    /// How many cells the hot buckets have set, together.
    hot_count: u32,
    /// How many cells each count tile of each bucket has set, by Morton
    /// index: kept in step with every change, as the buckets' counts
    /// are, and meaningful where they are. What sampling passes over
    /// most of a bitmap by, 128 bytes a bucket beside its 8 KiB.
    tile_counts: [[u16; COUNT_TILES_IN_CHUNK]; CHUNKS_IN_SUPERCHUNK],
}

impl SuperchunkLayer {
    /// How many cells the bucket at `chunk` has set.
    fn count(&self, chunk: usize) -> u32 {
        if contains(self.flags.nonempty, chunk) { self.counts_less_one[chunk] as u32 + 1 } else { 0 }
    }

    /// Makes `count` the bucket at `chunk`'s count of cells set.
    fn set_count(&mut self, chunk: usize, count: u32) {
        put(&mut self.flags.nonempty, chunk, count > 0);
        self.counts_less_one[chunk] = count.saturating_sub(1) as u16;
    }

    /// Words a bucket takes: a bitmap's, times the bits a cell.
    fn bucket_words(&self) -> usize {
        WORDS * self.layer_type.bits() as usize
    }

    /// Where the bucket of the chunk at `chunk` starts among the
    /// buckets, each `words` long, if it has one: after those of the
    /// chunks before it.
    #[inline]
    fn start(&self, chunk: usize, words: usize) -> Option<usize> {
        contains(self.kept, chunk).then(|| (self.kept & ((1 << chunk) - 1)).count_ones() as usize * words)
    }

    /// The words of the bucket of the chunk at `chunk`, however wide:
    /// all clear, if it has none.
    fn words(&self, chunk: usize) -> &[u64] {
        let words = self.bucket_words();
        match self.start(chunk, words) {
            Some(start) => &self.buckets[start..][..words],
            None => &NO_CELLS[..words],
        }
    }

    /// The bucket of the chunk at `chunk`, to change: made now, all
    /// clear and its counts none, in its place among the others, if the
    /// chunk had none.
    fn keep(&mut self, chunk: usize) -> &mut [u64] {
        let words = self.bucket_words();
        if !contains(self.kept, chunk) {
            put(&mut self.kept, chunk, true);
            let (start, end) = (self.start(chunk, words).expect("kept now"), self.buckets.len());
            self.buckets.reserve_exact(words);
            self.buckets.resize(end + words, 0);
            self.buckets.copy_within(start..end, start + words);
            self.buckets[start..][..words].fill(0);
            self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
            self.set_count(chunk, 0);
        }
        let start = self.start(chunk, words).expect("kept");
        &mut self.buckets[start..][..words]
    }

    /// Lets the bucket of the chunk at `chunk` go, if it has one: the
    /// chunk has no cell set then.
    fn let_bucket_go(&mut self, chunk: usize) {
        let words = self.bucket_words();
        if let Some(start) = self.start(chunk, words) {
            self.buckets.drain(start..start + words);
            put(&mut self.kept, chunk, false);
        }
        self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
        self.set_count(chunk, 0);
    }

    /// Lets go the bucket of every chunk neither hot nor waiting in the
    /// ring: nothing reads it any more.
    fn let_unused_go(&mut self) {
        for chunk in members(self.kept & !(self.flags.hot | self.flags.in_ring)) {
            self.let_bucket_go(chunk);
        }
    }

    /// The number at `cell`, in Morton order, of the bucket at `chunk`:
    /// the cell's bits, as many as the layer is wide.
    fn value(&self, chunk: usize, cell: usize) -> u32 {
        self.value_of(chunk, cell, self.layer_type.bits() as usize)
    }

    /// [`SuperchunkLayer::value`], the layer's `bits` a cell given:
    /// known where the plane's width is in its type.
    #[inline]
    fn value_of(&self, chunk: usize, cell: usize, bits: usize) -> u32 {
        debug_assert_eq!(bits, self.layer_type.bits() as usize, "a plane read at its own width");
        let Some(start) = self.start(chunk, WORDS * bits) else {
            return 0;
        };
        let at = cell * bits;
        (self.buckets[start + at / BITS_PER_WORD] >> (at % BITS_PER_WORD)) as u32 & ((1 << bits) - 1)
    }

    /// Makes `value` the number at `cell`, in Morton order, of the hot
    /// bucket at `chunk`, if it is not already: the bucket is then
    /// dirty, and the counts -- of the cells whose number is not 0 --
    /// move if the cell came to 0 or left it. Whether it changed.
    fn put_value(&mut self, chunk: usize, cell: usize, value: u32) -> bool {
        let (bits, at) = (self.layer_type.bits() as usize, cell * self.layer_type.bits() as usize);
        // A chunk with no bucket holds 0 everywhere: it is given one only for another number.
        let start = match self.start(chunk, WORDS * bits) {
            Some(start) => start,
            None if value as u64 & ((1u64 << bits) - 1) == 0 => return false,
            None => {
                self.keep(chunk);
                self.start(chunk, WORDS * bits).expect("kept now")
            }
        };
        let (word, mask) = (&mut self.buckets[start + at / BITS_PER_WORD], (1u64 << bits) - 1);
        let (was, value) = (*word >> (at % BITS_PER_WORD) & mask, value as u64 & mask);
        if was == value {
            return false;
        }
        *word ^= (was ^ value) << (at % BITS_PER_WORD);
        put(&mut self.flags.dirty, chunk, true);
        if (was == 0) != (value == 0) {
            let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
            if was == 0 {
                *tile_count += 1;
                self.set_count(chunk, count + 1);
                self.hot_count += 1;
            } else {
                *tile_count -= 1;
                self.set_count(chunk, count - 1);
                self.hot_count -= 1;
            }
        }
        true
    }

    /// The bucket of the chunk at `chunk`, of a layer a bit a cell: all
    /// clear, if it has none.
    fn cells(&self, chunk: usize) -> &CellWords {
        self.words(chunk).try_into().expect("a bucket is a bitmap's words")
    }

    /// Whether the cell at `cell`, in Morton order, of the bucket at
    /// `chunk` is set.
    #[inline]
    fn get(&self, chunk: usize, cell: usize) -> bool {
        self.start(chunk, WORDS).is_some_and(|start| self.buckets[start + cell / BITS_PER_WORD] >> (cell % BITS_PER_WORD) & 1 == 1)
    }

    /// Makes the cell at `cell`, in Morton order, of the hot bucket at
    /// `chunk` set or clear, if it is not already: the bucket is then
    /// dirty, and its count and the hot count move by one. Whether it
    /// changed.
    fn put_cell(&mut self, chunk: usize, cell: usize, set: bool) -> bool {
        // A chunk with no bucket has no cell set: it is given one only to set one.
        let start = match self.start(chunk, WORDS) {
            Some(start) => start,
            None if !set => return false,
            None => {
                self.keep(chunk);
                self.start(chunk, WORDS).expect("kept now")
            }
        };
        // The one word read and written, found once.
        let (word, bit) = (&mut self.buckets[start + cell / BITS_PER_WORD], 1 << (cell % BITS_PER_WORD));
        if (*word & bit != 0) == set {
            return false;
        }
        *word ^= bit;
        put(&mut self.flags.dirty, chunk, true);
        let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
        if set {
            *tile_count += 1;
            self.set_count(chunk, count + 1);
            self.hot_count += 1;
        } else {
            *tile_count -= 1;
            self.set_count(chunk, count - 1);
            self.hot_count -= 1;
        }
        true
    }
}

/// One hot bitmap, to read.
pub struct Bucket<'a> {
    /// Its cells' words: a bitmap's, times its layer's bits a cell.
    words: &'a [u64],
    /// How many of them are set.
    count: u32,
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
    index: SuperchunkIndex,
    /// Its allocations, sorted by layer type, one a type.
    layers: Vec<SuperchunkLayer>,
    /// Its write-backs taken ([`BitmapArena::take_dirty`]) and not yet
    /// in the ring ([`BitmapArena::written_back`]): while any is on its
    /// way, its buckets are newer than chunk storage.
    on_their_way: u32,
}

impl Superchunk {
    /// Where `layer_type` is among the layers, if it has one.
    fn layer_index(&self, layer_type: LayerType) -> Option<usize> {
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

/// Reads cells from superchunks, remembering its last lookup -- one a
/// thread -- so runs of reads in one superchunk search nothing.
pub struct Reader<'a> {
    /// The superchunks read, sorted by superchunk index.
    superchunks: &'a [Superchunk],
    /// The lookups, remembering the last.
    lookup: Lookup,
}

impl<'a> Reader<'a> {
    /// A reader of `superchunks`: an arena's ([`BitmapArena::superchunks`]).
    pub fn new(superchunks: &'a [Superchunk]) -> Self {
        Self { superchunks, lookup: Lookup::default() }
    }

    /// Whether `layer_type` holds at `cell`: its superchunk, chunk and
    /// bit taken from its index.
    pub fn holds(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.lookup.holds(self.superchunks, layer_type, cell)
    }

    /// The number `plane` holds at `cell`: one read, however many bits.
    #[inline]
    pub fn value<W: Width>(&self, plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.lookup.value(self.superchunks, plane, cell)
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of `layer_type`, row by row ([`Window`]): one
    /// to four bitmap words read, turned and cut, so a cell's whole
    /// neighbourhood, say, is a few masks.
    pub fn window(&self, layer_type: LayerType, origin: CellIndex, width: u32, height: u32) -> Window {
        let [tile] = self.windows([layer_type], origin, width, height);
        tile
    }

    /// [`Reader::window`], of each of `types` at once: one window of
    /// cells read in several layers costs little more than in one, where
    /// it lies being worked out once.
    pub fn windows<const N: usize>(&self, types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        self.lookup.windows(self.superchunks, types, origin, width, height)
    }

    /// Asks memory for the word `cell` is in, of `layer_type`, ahead of
    /// its being read: nothing if its bitmap is not hot.
    pub fn prefetch(&self, layer_type: LayerType, cell: CellIndex) {
        if let Some(cells) = self.lookup.bucket(self.superchunks, layer_type, cell) {
            prefetch(&cells[cell.place() / BITS_PER_WORD]);
        }
    }

    /// Whether `layer_type` holds at any cell of the tile of `scale`
    /// that `cell` is in, up to [`COARSEST_SCALE`]: a run of bits in
    /// Morton order, passed over by its count tiles' counts where they
    /// are 0, so the world is looked at from far off for little. `None`
    /// if its bitmap is not hot.
    pub fn any_in_tile(&self, layer_type: LayerType, cell: CellIndex, scale: u32) -> Option<bool> {
        self.lookup.any_in_tile(self.superchunks, layer_type, cell, scale)
    }

    /// Which of the [`COARSEST_TILES_IN_CHUNK`] tiles of the coarsest
    /// scale in `cell`'s chunk -- in Morton order, a bit each --
    /// `layer_type` holds at any cell of: read off the counts, no cell
    /// looked at, and an empty chunk off its flag. `None` if its bitmap
    /// is not hot.
    pub fn tiles_holding(&self, layer_type: LayerType, cell: CellIndex) -> Option<u16> {
        self.lookup.tiles_holding(self.superchunks, layer_type, cell)
    }

    /// Where `superchunk` is among the superchunks, if there.
    pub fn superchunk(&self, superchunk: SuperchunkIndex) -> Option<usize> {
        self.lookup.superchunk(self.superchunks, superchunk).ok()
    }
}

/// Up to 8x8 cells of one layer type, row by row: cell `(x, y)` from
/// the window's top left at bit `y * 8 + x` ([`bitmap::window`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// The cells the type holds at: hot ones only.
    pub set: u64,
    /// The cells in hot bitmaps: in the world, read, and in the window.
    pub hot: u64,
}

/// A word tile's index in its chunk -- its word's index in the bitmap --
/// is a Morton index over the chunk's 32x32 word tiles: its x in the
/// even bits...
const WORD_TILE_X: usize = 0x155;
/// ...and its y in the odd ones.
const WORD_TILE_Y: usize = 0x2aa;

/// The word tile at `tile` of `bucket`, row by row; nothing if not hot.
fn word_tile(bucket: Option<&CellWords>, tile: usize) -> Window {
    match bucket {
        Some(cells) => Window { set: rows_from_morton(cells[tile]), hot: u64::MAX },
        None => Window::default(),
    }
}

/// Where an allocation is: its superchunk's entry in the directory, and
/// its index among the entry's allocations.
type LayerAt = (usize, usize);

/// Lookups in the directory, remembering the last superchunk found:
/// runs of lookups in one superchunk -- a rule reading grass and dirt
/// by turns -- search nothing but its few layers. One a thread: each
/// remembers its own.
#[derive(Default)]
struct Lookup {
    /// The last superchunk looked up, and its entry.
    superchunk: Cell<Option<(SuperchunkIndex, usize)>>,
}

impl Lookup {
    /// Forgets the superchunk remembered: the directory's shape changed.
    fn forget(&self) {
        self.superchunk.set(None);
    }

    /// Where `superchunk` is in `directory`, or where it would go.
    fn superchunk(&self, directory: &[Superchunk], superchunk: SuperchunkIndex) -> Result<usize, usize> {
        if let Some((last, entry)) = self.superchunk.get()
            && last == superchunk
        {
            return Ok(entry);
        }
        let found = directory.binary_search_by_key(&superchunk, |entry| entry.index);
        if let Ok(entry) = found {
            self.superchunk.set(Some((superchunk, entry)));
        }
        found
    }

    /// Where the allocation for `layer_type` over `superchunk` is in
    /// `directory`, if in use.
    fn find(&self, directory: &[Superchunk], layer_type: LayerType, superchunk: SuperchunkIndex) -> Option<LayerAt> {
        let entry = self.superchunk(directory, superchunk).ok()?;
        directory[entry].layer_index(layer_type).map(|layer| (entry, layer))
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of each of `types` in `directory`, row by
    /// row: put together from the up to four word tiles it overlaps,
    /// only those it reaches read. Where the window lies among them is
    /// worked out once, for every type; each type's bucket is looked up
    /// once, the word tiles beside and below stepped to on the word
    /// tile's index in the chunk, and only one across the chunk's edge
    /// looked up again.
    fn windows<const N: usize>(&self, directory: &[Superchunk], types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        // The window's top left in its word tile.
        let (across, down) = in_word_tile(origin.0);
        let first = CellIndex(origin.0 & !PLACE_IN_WORD_TILE);
        let tile = first.place() / BITS_PER_WORD;
        let (x, y) = (tile & WORD_TILE_X, tile & WORD_TILE_Y);
        // The word tiles beside and below, in the chunk: a carry through the other coordinate's bits.
        let (beside, below) = (((x | WORD_TILE_Y) + 1) & WORD_TILE_X, ((y | WORD_TILE_X) + 2) & WORD_TILE_Y);
        let (wide, tall) = (across + width > WORD_TILE_SIDE, down + height > WORD_TILE_SIDE);
        let side = WORD_TILE_SIDE as i32;
        let kept = left_columns(width) & top_rows(height);
        types.map(|layer_type| {
            let bucket = self.bucket(directory, layer_type, first);
            let top_left = word_tile(bucket, tile);
            let (mut top_right, mut bottom_left, mut bottom_right) = (Window::default(), Window::default(), Window::default());
            if wide {
                top_right = if x != WORD_TILE_X { word_tile(bucket, beside | y) } else { self.word_tile_at(directory, layer_type, first.offset(side, 0)) };
            }
            if tall {
                bottom_left = if y != WORD_TILE_Y { word_tile(bucket, x | below) } else { self.word_tile_at(directory, layer_type, first.offset(0, side)) };
            }
            if wide && tall {
                bottom_right = if x != WORD_TILE_X && y != WORD_TILE_Y {
                    word_tile(bucket, beside | below)
                } else {
                    self.word_tile_at(directory, layer_type, first.offset(side, side))
                };
            }
            Window {
                set: window([[top_left.set, top_right.set], [bottom_left.set, bottom_right.set]], across, down) & kept,
                hot: window([[top_left.hot, top_right.hot], [bottom_left.hot, bottom_right.hot]], across, down) & kept,
            }
        })
    }

    /// The hot bucket of `cell`'s chunk in `layer_type`, in `directory`.
    fn bucket<'d>(&self, directory: &'d [Superchunk], layer_type: LayerType, cell: CellIndex) -> Option<&'d CellWords> {
        let chunk = cell.chunk().place();
        let (entry, layer) = self.find(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry].layers[layer];
        contains(layer.flags.hot, chunk).then(|| layer.cells(chunk))
    }

    /// Whether `layer_type` holds, in `directory`, at any cell of the
    /// tile of `scale` that `cell` is in; `None` if its bitmap is not hot.
    fn any_in_tile(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex, scale: u32) -> Option<bool> {
        debug_assert!(scale <= COARSEST_SCALE, "a tile coarser than the coarsest scale");
        let chunk = cell.chunk().place();
        let (entry, layer) = self.find(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry].layers[layer];
        if !contains(layer.flags.hot, chunk) {
            return None;
        }
        // The tile's cells are a run of this many bits, in Morton order.
        let run = 1usize << (2 * scale);
        let first = cell.place() & !(run - 1);
        // The count tiles the run lies in: one, or those it is made of.
        let counted = &layer.tile_counts[chunk][first / COUNT_TILE_CELLS..][..run.div_ceil(COUNT_TILE_CELLS)];
        if counted.iter().all(|&count| count == 0) {
            return Some(false);
        }
        let cells = layer.cells(chunk);
        Some(if run < BITS_PER_WORD {
            cells[first / BITS_PER_WORD] >> (first % BITS_PER_WORD) & ((1 << run) - 1) != 0
        } else {
            run >= COUNT_TILE_CELLS || cells[first / BITS_PER_WORD..(first + run) / BITS_PER_WORD].iter().any(|&word| word != 0)
        })
    }

    /// Which tiles of the coarsest scale in `cell`'s chunk `layer_type`
    /// holds at any cell of, in `directory`, a bit each; `None` if its
    /// bitmap is not hot.
    fn tiles_holding(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex) -> Option<u16> {
        let chunk = cell.chunk().place();
        let (entry, layer) = self.find(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry].layers[layer];
        if !contains(layer.flags.hot, chunk) {
            return None;
        }
        if !contains(layer.flags.nonempty, chunk) {
            return Some(0);
        }
        Some(layer.tile_counts[chunk].as_chunks::<COUNTS_IN_COARSEST>().0.iter().enumerate().fold(0, |holding, (tile, counts)| holding | (counts.iter().any(|&count| count != 0) as u16) << tile))
    }

    /// The word tile whose first cell is `first`, if in the world, of
    /// `layer_type` in `directory`: looked up.
    fn word_tile_at(&self, directory: &[Superchunk], layer_type: LayerType, first: Option<CellIndex>) -> Window {
        first.map_or(Window::default(), |first| word_tile(self.bucket(directory, layer_type, first), first.place() / BITS_PER_WORD))
    }

    /// The number `plane` holds at `cell` in `directory`.
    #[inline]
    fn value<W: Width>(&self, directory: &[Superchunk], plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        let (layer_type, chunk) = (plane.layer_type(), cell.chunk().place());
        match self.find(directory, layer_type, cell.superchunk()) {
            Some((entry, layer)) if contains(directory[entry].layers[layer].flags.hot, chunk) => Ok(directory[entry].layers[layer].value_of(chunk, cell.place(), W::BITS as usize)),
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }

    /// Whether `layer_type` holds at `cell` in `directory`: its
    /// superchunk, chunk and bit taken from its index.
    fn holds(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        let chunk = cell.chunk().place();
        match self.find(directory, layer_type, cell.superchunk()) {
            Some((entry, layer)) if contains(directory[entry].layers[layer].flags.hot, chunk) => {
                Ok(directory[entry].layers[layer].get(chunk, cell.place()))
            }
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }
}

/// A superchunk gone cold as a whole ([`BitmapArena::make_cold_superchunk`]):
/// its allocations kept as they were -- hot buckets and all, counted --
/// until chunk storage holds its changes, or for as long as it is
/// wanted hot again, when it is made hot as it is, nothing decoded
/// ([`BitmapArena::make_hot_again`]).
struct Lingering {
    /// The superchunk, as it was when it went cold.
    superchunk: Superchunk,
    /// Whether it is wanted hot again: kept until it is.
    wanted: bool,
}

impl Lingering {
    /// Whether it can be let go: unwanted, and every change of it in
    /// chunk storage's cold pool -- none on its way, none in the ring.
    fn done(&self) -> bool {
        !self.wanted && self.superchunk.on_their_way == 0 && self.superchunk.layers.iter().all(|layer| layer.flags.in_ring == 0)
    }
}

/// The hot bitmaps, in allocations a superchunk each.
pub struct BitmapArena {
    /// Every superchunk with a layer in use, sorted by superchunk index:
    /// the hot ones.
    directory: Vec<Superchunk>,
    /// The superchunks gone cold whose allocations are kept, sorted by
    /// superchunk index.
    lingering: Vec<Lingering>,
    /// The arena's own lookups, remembering the last.
    lookup: Lookup,
    /// Writes queued from outside a tick, not yet applied.
    queued: WriteQueues,
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
    fn layers(&self) -> impl Iterator<Item = &SuperchunkLayer> {
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
    fn at_mut(&mut self, (entry, layer): LayerAt) -> &mut SuperchunkLayer {
        &mut self.directory[entry].layers[layer]
    }

    /// Where `key`'s bucket is, if it is hot: its allocation, and the
    /// bucket's index there.
    fn hot(&self, key: BucketKey) -> Option<(LayerAt, usize)> {
        let at = self.lookup.find(&self.directory, key.layer_type, key.chunk.superchunk())?;
        contains(self.at(at).flags.hot, key.chunk.place()).then_some((at, key.chunk.place()))
    }

    /// Whether `key`'s bitmap is hot.
    pub fn is_hot(&self, key: BucketKey) -> bool {
        self.hot(key).is_some()
    }

    /// The allocation for `layer_type` over `superchunk`: the one in
    /// use, or else a new one, with no bucket yet.
    fn allocation(&mut self, layer_type: LayerType, superchunk: SuperchunkIndex) -> LayerAt {
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

    /// Makes `key`'s bitmap hot, decoding `layer` -- the encoded bitmap
    /// of that type in the chunk, from its first word on, `None` if it
    /// has none, which is a bitmap with no cell set. A bitmap already hot
    /// is left as it is, changes and all, and one waiting in the ring is
    /// made hot as its bucket holds it, not decoded: whether it was made
    /// hot now.
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

    /// Makes every bitmap over `superchunk` cold at once, nothing
    /// encoded or flushed: its dirty buckets taken
    /// ([`BitmapArena::take_dirty`]) and returned, to be encoded and
    /// [`BitmapArena::written_back`]; its allocations set aside as they
    /// are, lingering, until chunk storage holds its changes -- and so
    /// longer, if it is wanted hot again ([`BitmapArena::hold`]).
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
    fn lingering_at(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
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
    fn release_unused(&mut self) {
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
