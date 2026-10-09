//! Cells read wherever they lie: a reader over the superchunks, the
//! windows it reads, and the lookups in the directory behind it.

use super::{COARSEST_SCALE, COUNTS_IN_COARSEST, COUNT_TILE_CELLS, NotHot, contains};
use super::superchunk::Superchunk;
use bitmap::{BITS_PER_WORD, CellWords};
use bitmap::window::{PLACE_IN_WORD_TILE, WORD_TILE_SIDE, in_word_tile, left_columns, rows_from_morton, top_rows, window};
use chunk_storage::{BucketKey, LayerType, Wide, Width};
use coordinates::{CellIndex, SuperchunkIndex};
use std::cell::Cell;
use utilities::cache::prefetch;

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
pub(crate) type LayerAt = (usize, usize);

/// Lookups in the directory, remembering the last superchunk found:
/// runs of lookups in one superchunk -- a rule reading grass and dirt
/// by turns -- search nothing but its few layers. One a thread: each
/// remembers its own.
#[derive(Default)]
pub(crate) struct Lookup {
    /// The last superchunk looked up, and its entry.
    superchunk: Cell<Option<(SuperchunkIndex, usize)>>,
}

impl Lookup {
    /// Forgets the superchunk remembered: the directory's shape changed.
    pub(crate) fn forget(&self) {
        self.superchunk.set(None);
    }

    /// Where `superchunk` is in `directory`, or where it would go.
    pub(crate) fn superchunk(&self, directory: &[Superchunk], superchunk: SuperchunkIndex) -> Result<usize, usize> {
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
    pub(crate) fn find(&self, directory: &[Superchunk], layer_type: LayerType, superchunk: SuperchunkIndex) -> Option<LayerAt> {
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
    pub(crate) fn value<W: Width>(&self, directory: &[Superchunk], plane: Wide<W>, cell: CellIndex) -> Result<u32, NotHot> {
        let (layer_type, chunk) = (plane.layer_type(), cell.chunk().place());
        match self.find(directory, layer_type, cell.superchunk()) {
            Some((entry, layer)) if contains(directory[entry].layers[layer].flags.hot, chunk) => Ok(directory[entry].layers[layer].value_of(chunk, cell.place(), W::BITS as usize)),
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }

    /// Whether `layer_type` holds at `cell` in `directory`: its
    /// superchunk, chunk and bit taken from its index.
    pub(crate) fn holds(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        let chunk = cell.chunk().place();
        match self.find(directory, layer_type, cell.superchunk()) {
            Some((entry, layer)) if contains(directory[entry].layers[layer].flags.hot, chunk) => {
                Ok(directory[entry].layers[layer].get(chunk, cell.place()))
            }
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }
}
