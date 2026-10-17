//! Cells read wherever they lie: a reader over the superchunks, the
//! windows it reads, and the lookups in the directory behind it.

pub(crate) mod aligned_tiles;

use super::{COARSEST_SCALE, COUNTS_IN_COARSEST, COUNT_TILE_CELLS, NotHot, contains};
use super::superchunk::Superchunk;
use utilities::bitmap::{BITS_PER_WORD, CellWords};
use utilities::bitmap::window::{PLACE_IN_WORD_TILE, WORD_TILE_SIDE, in_word_tile, left_columns, rows_from_morton, top_rows, window};
use chunk_storage::{BucketKey, LayerType, WidePlane, Width};
use utilities::coordinates::{CellIndex, SuperchunkIndex};
use std::cell::Cell;
use utilities::cache::prefetch_cache_line;

/// Reads cells from superchunks, remembering its last lookup -- one a
/// thread -- so runs of reads in one superchunk search nothing.
pub struct CellReader<'a> {
    /// The superchunks read, sorted by superchunk index.
    superchunks: &'a [Superchunk],
    /// The lookups, remembering the last.
    lookup: Lookup,
}

impl<'a> CellReader<'a> {
    /// A reader of `superchunks`: an arena's ([`BitmapArena::superchunks`]).
    pub fn new(superchunks: &'a [Superchunk]) -> Self {
        Self { superchunks, lookup: Lookup::default() }
    }

    /// Whether `layer_type` holds at `cell`: its superchunk, chunk and
    /// bit taken from its index.
    pub fn layer_holds_cell(&self, layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        self.lookup.layer_holds_cell(self.superchunks, layer_type, cell)
    }

    /// The number `wide_plane` holds at `cell`: one read, however many bits.
    #[inline]
    pub fn value<W: Width>(&self, wide_plane: WidePlane<W>, cell: CellIndex) -> Result<u32, NotHot> {
        self.lookup.value(self.superchunks, wide_plane, cell)
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of `layer_type`, row by row ([`Window`]): one
    /// to four bitmap words read, turned and cut, so a cell's whole
    /// neighbourhood, say, is a few masks.
    pub fn window(&self, layer_type: LayerType, origin: CellIndex, width: u32, height: u32) -> Window {
        let [tile] = self.windows([layer_type], origin, width, height);
        tile
    }

    /// [`CellReader::window`], of each of `layer_types` at once: one window of
    /// cells read in several layers costs little more than in one, where
    /// it lies being worked out once.
    #[inline]
    pub fn windows<const N: usize>(&self, layer_types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        self.lookup.windows(self.superchunks, layer_types, origin, width, height)
    }

    /// Asks memory for the word `cell` is in, of `layer_type`, ahead of
    /// its being read: nothing if its bitmap is not hot.
    pub fn prefetch_cell(&self, layer_type: LayerType, cell: CellIndex) {
        if let Some(cells) = self.lookup.bucket(self.superchunks, layer_type, cell) {
            prefetch_cache_line(&cells[cell.place() / BITS_PER_WORD]);
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
/// the window's top left at bit `y * 8 + x` ([`utilities::bitmap::window`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// The cells the type holds at: hot ones only.
    pub cells_set: u64,
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
        Some(cells) => Window { cells_set: rows_from_morton(cells[tile]), hot: u64::MAX },
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
    pub(crate) fn forget_last(&self) {
        self.superchunk.set(None);
    }

    /// Where `superchunk` is in `directory`, or where it would go.
    pub(crate) fn superchunk(&self, directory: &[Superchunk], superchunk: SuperchunkIndex) -> Result<usize, usize> {
        if let Some((last_superchunk, last_entry_index)) = self.superchunk.get()
            && last_superchunk == superchunk
        {
            return Ok(last_entry_index);
        }
        let search_result = directory.binary_search_by_key(&superchunk, |directory_entry| directory_entry.superchunk_index);
        if let Ok(found_entry_index) = search_result {
            self.superchunk.set(Some((superchunk, found_entry_index)));
        }
        search_result
    }

    /// Where the allocation for `layer_type` over `superchunk` is in
    /// `directory`, if in use.
    pub(crate) fn find_allocation(&self, directory: &[Superchunk], layer_type: LayerType, superchunk: SuperchunkIndex) -> Option<LayerAt> {
        let entry_index = self.superchunk(directory, superchunk).ok()?;
        directory[entry_index].layer_index(layer_type).map(|layer| (entry_index, layer))
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of each of `layer_types` in `directory`, row by
    /// row (`docs/bitplane_manager.md`, "The arena", Windows).
    #[inline]
    fn windows<const N: usize>(&self, directory: &[Superchunk], layer_types: [LayerType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        // The window's top left in its word tile.
        let (across_in_word_tile, down_in_word_tile) = in_word_tile(origin.0);
        let tile_first_cell = CellIndex(origin.0 & !PLACE_IN_WORD_TILE);
        let tile = tile_first_cell.place() / BITS_PER_WORD;
        let (x, y) = (tile & WORD_TILE_X, tile & WORD_TILE_Y);
        // The word tiles beside and below, in the chunk: a carry through the other coordinate's bits.
        let (tile_beside, tile_below) = (((x | WORD_TILE_Y) + 1) & WORD_TILE_X, ((y | WORD_TILE_X) + 2) & WORD_TILE_Y);
        let (past_tile_across, past_tile_down) = (across_in_word_tile + width > WORD_TILE_SIDE, down_in_word_tile + height > WORD_TILE_SIDE);
        let word_tile_side = WORD_TILE_SIDE as i32;
        let window_mask = left_columns(width) & top_rows(height);
        // Built by index: an array mapped by a closure is drained through calls the optimizer leaves standing.
        std::array::from_fn(|type_index| {
            let layer_type = layer_types[type_index];
            let bucket = self.bucket(directory, layer_type, tile_first_cell);
            let top_left = word_tile(bucket, tile);
            let (mut top_right, mut bottom_left, mut bottom_right) = (Window::default(), Window::default(), Window::default());
            if past_tile_across {
                top_right = if x != WORD_TILE_X { word_tile(bucket, tile_beside | y) } else { self.word_tile_at(directory, layer_type, tile_first_cell.at_cartesian_offset(word_tile_side, 0)) };
            }
            if past_tile_down {
                bottom_left = if y != WORD_TILE_Y { word_tile(bucket, x | tile_below) } else { self.word_tile_at(directory, layer_type, tile_first_cell.at_cartesian_offset(0, word_tile_side)) };
            }
            if past_tile_across && past_tile_down {
                bottom_right = if x != WORD_TILE_X && y != WORD_TILE_Y {
                    word_tile(bucket, tile_beside | tile_below)
                } else {
                    self.word_tile_at(directory, layer_type, tile_first_cell.at_cartesian_offset(word_tile_side, word_tile_side))
                };
            }
            Window {
                cells_set: window([[top_left.cells_set, top_right.cells_set], [bottom_left.cells_set, bottom_right.cells_set]], across_in_word_tile, down_in_word_tile) & window_mask,
                hot: window([[top_left.hot, top_right.hot], [bottom_left.hot, bottom_right.hot]], across_in_word_tile, down_in_word_tile) & window_mask,
            }
        })
    }

    /// The hot bucket of `cell`'s chunk in `layer_type`, in `directory`.
    fn bucket<'d>(&self, directory: &'d [Superchunk], layer_type: LayerType, cell: CellIndex) -> Option<&'d CellWords> {
        let chunk = cell.chunk().place();
        let (entry_index, layer) = self.find_allocation(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry_index].layers[layer];
        contains(layer.chunk_flags.hot, chunk).then(|| layer.cells(chunk))
    }

    /// Whether `layer_type` holds, in `directory`, at any cell of the
    /// tile of `scale` that `cell` is in; `None` if its bitmap is not hot.
    fn any_in_tile(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex, scale: u32) -> Option<bool> {
        debug_assert!(scale <= COARSEST_SCALE, "a tile coarser than the coarsest scale");
        let chunk = cell.chunk().place();
        let (entry_index, layer) = self.find_allocation(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry_index].layers[layer];
        if !contains(layer.chunk_flags.hot, chunk) {
            return None;
        }
        // The tile's cells are a run of this many bits, in Morton order.
        let cells_in_tile = 1usize << (2 * scale);
        let tile_first_place = cell.place() & !(cells_in_tile - 1);
        // The count tiles the run lies in: one, or those it is made of.
        let covering_tile_counts = &layer.tile_counts[chunk][tile_first_place / COUNT_TILE_CELLS..][..cells_in_tile.div_ceil(COUNT_TILE_CELLS)];
        if covering_tile_counts.iter().all(|&count| count == 0) {
            return Some(false);
        }
        let cells = layer.cells(chunk);
        Some(if cells_in_tile < BITS_PER_WORD {
            cells[tile_first_place / BITS_PER_WORD] >> (tile_first_place % BITS_PER_WORD) & ((1 << cells_in_tile) - 1) != 0
        } else {
            cells_in_tile >= COUNT_TILE_CELLS || cells[tile_first_place / BITS_PER_WORD..(tile_first_place + cells_in_tile) / BITS_PER_WORD].iter().any(|&word| word != 0)
        })
    }

    /// Which tiles of the coarsest scale in `cell`'s chunk `layer_type`
    /// holds at any cell of, in `directory`, a bit each; `None` if its
    /// bitmap is not hot.
    fn tiles_holding(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex) -> Option<u16> {
        let chunk = cell.chunk().place();
        let (entry_index, layer) = self.find_allocation(directory, layer_type, cell.superchunk())?;
        let layer = &directory[entry_index].layers[layer];
        if !contains(layer.chunk_flags.hot, chunk) {
            return None;
        }
        if !contains(layer.chunk_flags.with_cells_set, chunk) {
            return Some(0);
        }
        Some(layer.tile_counts[chunk].as_chunks::<COUNTS_IN_COARSEST>().0.iter().enumerate().fold(0, |holding_tiles, (tile, counts)| holding_tiles | (counts.iter().any(|&count| count != 0) as u16) << tile))
    }

    /// The word tile whose first cell is `tile_first_cell_in_world`, if in the world, of
    /// `layer_type` in `directory`: looked up.
    fn word_tile_at(&self, directory: &[Superchunk], layer_type: LayerType, tile_first_cell_in_world: Option<CellIndex>) -> Window {
        tile_first_cell_in_world.map_or(Window::default(), |tile_first_cell| word_tile(self.bucket(directory, layer_type, tile_first_cell), tile_first_cell.place() / BITS_PER_WORD))
    }

    /// The number `wide_plane` holds at `cell` in `directory`.
    #[inline]
    pub(crate) fn value<W: Width>(&self, directory: &[Superchunk], wide_plane: WidePlane<W>, cell: CellIndex) -> Result<u32, NotHot> {
        let (layer_type, chunk) = (wide_plane.layer_type(), cell.chunk().place());
        match self.find_allocation(directory, layer_type, cell.superchunk()) {
            Some((entry_index, layer)) if contains(directory[entry_index].layers[layer].chunk_flags.hot, chunk) => Ok(directory[entry_index].layers[layer].value_of(chunk, cell.place(), W::BITS_A_CELL as usize)),
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }

    /// Whether `layer_type` holds at `cell` in `directory`: its
    /// superchunk, chunk and bit taken from its index.
    pub(crate) fn layer_holds_cell(&self, directory: &[Superchunk], layer_type: LayerType, cell: CellIndex) -> Result<bool, NotHot> {
        let chunk = cell.chunk().place();
        match self.find_allocation(directory, layer_type, cell.superchunk()) {
            Some((entry_index, layer)) if contains(directory[entry_index].layers[layer].chunk_flags.hot, chunk) => {
                Ok(directory[entry_index].layers[layer].get(chunk, cell.place()))
            }
            _ => Err(NotHot(BucketKey { layer_type, chunk: cell.chunk() })),
        }
    }
}
