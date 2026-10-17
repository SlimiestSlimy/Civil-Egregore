//! Masks: an aligned tile's cells as bits, [`MASK_SIDES`] a side, in
//! Morton order -- as the tile lies in its bitmap, one run of bits --
//! so a layer is read into one by copying words, asked under one with
//! an AND, and written under one a set bit a cell
//! (`docs/actions.md`, "Masks").

use crate::simulation_api::cell_and_its_neighbours::set_bit_of_rank;
use bitplane_manager::{SMALL_TILE_CELLS, SMALL_TILE_SIDE};
use chunk_storage::LayerType;
use utilities::coordinates::{cartesian_from_place, place_from_cartesian, CellIndex, CHUNK_SIDE};
use crate::SimulationTickState;
use std::ops::Range;
use utilities::rng::RandomSource;

/// The sides a mask comes in: powers of two from a small tile's 4 to
/// a superchunk's, an entity's reach.
pub const MASK_SIDES: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// Bits in a word of a mask.
const WORD: u32 = u64::BITS;
/// The bits of a small tile, the one mask that is less than a word.
const SMALL_TILE: u64 = (1 << SMALL_TILE_CELLS) - 1;

/// An aligned tile of cells, a bit each: the cell `(x, y)` from the
/// tile's top left at the bit whose index is the two interleaved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// Cells along its side: one of [`MASK_SIDES`].
    side_in_cells: u32,
    /// Its cells in Morton order, a cell's bit clear past the tile's --
    /// which only a small tile's word has any past.
    words: Vec<u64>,
}

impl Mask {
    /// A mask `side_in_cells` cells a side, no cell set.
    pub fn all_clear(side_in_cells: u32) -> Self {
        assert!(MASK_SIDES.contains(&side_in_cells), "a mask {side_in_cells} cells a side: not one of {MASK_SIDES:?}");
        Self { side_in_cells, words: vec![0; (side_in_cells * side_in_cells).div_ceil(WORD) as usize] }
    }

    /// A mask `side_in_cells` cells a side, every cell set.
    pub fn all_set(side_in_cells: u32) -> Self {
        let mut mask = Self::all_clear(side_in_cells);
        mask.words.fill(if side_in_cells == SMALL_TILE_SIDE { SMALL_TILE } else { u64::MAX });
        mask
    }

    /// A mask `side_in_cells` cells a side, the cells set whose centres are no
    /// farther than half the side from the tile's: the disc that
    /// fills it.
    pub fn filling_disc(side_in_cells: u32) -> Self {
        let mut mask = Self::all_clear(side_in_cells);
        // In half cells, from the tile's centre to a cell's.
        let reach = side_in_cells as i64 * side_in_cells as i64;
        for (x, y) in (0..side_in_cells).flat_map(|y| (0..side_in_cells).map(move |x| (x, y))) {
            let (dx, dy) = (2 * x as i64 + 1 - side_in_cells as i64, 2 * y as i64 + 1 - side_in_cells as i64);
            if dx * dx + dy * dy <= reach {
                mask.set(x, y, true);
            }
        }
        mask
    }

    /// Cells along its side.
    pub fn side_in_cells(&self) -> u32 {
        self.side_in_cells
    }

    /// Its cells in Morton order, a word 8x8 of them: as the tile's
    /// lie in a bitmap.
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// Whether the cell `(x, y)` is set.
    pub fn get(&self, x: u32, y: u32) -> bool {
        let cell_place = place_from_cartesian(x, y);
        self.words[cell_place / WORD as usize] >> (cell_place % WORD as usize) & 1 == 1
    }

    /// Makes the cell `(x, y)` set, or clear.
    pub fn set(&mut self, x: u32, y: u32, is_set: bool) {
        debug_assert!(x < self.side_in_cells && y < self.side_in_cells, "({x}, {y}) of a mask {} a side", self.side_in_cells);
        let cell_place = place_from_cartesian(x, y);
        let word = &mut self.words[cell_place / WORD as usize];
        *word = *word & !(1 << (cell_place % WORD as usize)) | (is_set as u64) << (cell_place % WORD as usize);
    }

    /// Clears every cell.
    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    /// How many cells are set.
    pub fn count(&self) -> u32 {
        self.words.iter().map(|word| word.count_ones()).sum()
    }

    /// Whether no cell is set.
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&word| word == 0)
    }

    /// Keeps the cells `other_mask`, of the same side, has set too.
    pub fn and_with(&mut self, other_mask: &Mask) {
        self.combine_words_with(other_mask, |word, other_word| word & other_word);
    }

    /// Sets the cells `other_mask`, of the same side, has set.
    pub fn or_with(&mut self, other_mask: &Mask) {
        self.combine_words_with(other_mask, |word, other_word| word | other_word);
    }

    /// Clears the cells `other_mask`, of the same side, has set.
    pub fn and_not(&mut self, other_mask: &Mask) {
        self.combine_words_with(other_mask, |word, other_word| word & !other_word);
    }

    /// Each word made `combine_words` it and `other_mask`'s.
    fn combine_words_with(&mut self, other_mask: &Mask, combine_words: impl Fn(u64, u64) -> u64) {
        assert_eq!(self.side_in_cells, other_mask.side_in_cells, "masks of two sides");
        for (word, &other_word) in self.words.iter_mut().zip(&other_mask.words) {
            *word = combine_words(*word, other_word);
        }
    }

    /// The set cells' bits, in Morton order: each its cell's place in
    /// the tile.
    fn set_bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(move |(word_index, &word)| {
            std::iter::successors((word != 0).then_some(word), |&bits_left| Some(bits_left & (bits_left - 1)).filter(|&fewer_bits_left| fewer_bits_left != 0)).map(move |remaining_bits| word_index * WORD as usize + remaining_bits.trailing_zeros() as usize)
        })
    }

    /// The set cells, `(x, y)`, in Morton order.
    pub fn cells(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.set_bits().map(cartesian_from_place)
    }

    /// One of the set cells, `(x, y)`, drawn from `random_stream`: none if
    /// none is set, and then nothing is drawn.
    pub fn pick_set_cell(&self, random_stream: &mut RandomSource) -> Option<(u32, u32)> {
        let count = self.count();
        if count == 0 {
            return None;
        }
        let mut set_bit_rank = random_stream.draw_below(count as u64) as u32;
        for (word_index, &word) in self.words.iter().enumerate() {
            if set_bit_rank >= word.count_ones() {
                set_bit_rank -= word.count_ones();
                continue;
            }
            return Some(cartesian_from_place(word_index * WORD as usize + set_bit_of_rank(word, set_bit_rank) as usize));
        }
        None
    }
}

/// The first cell -- the top left one -- of the aligned tile `tile_side`
/// cells a side that `cell_in_tile` is in: the tile a mask of that side stands
/// for there.
pub fn tile(cell_in_tile: CellIndex, tile_side: u32) -> CellIndex {
    CellIndex(cell_in_tile.0 & !(u64::from(tile_side) * u64::from(tile_side) - 1))
}

/// The cell `(x, y)` of the tile whose first cell is `tile_first_cell`.
pub fn cell(tile_first_cell: CellIndex, x: u32, y: u32) -> CellIndex {
    CellIndex(tile_first_cell.0 | place_from_cartesian(x, y) as u64)
}

/// The tile `tile_side` cells a side (8 at least) whose first cell is
/// `tile_first_cell`, of `layer_type`, as the tick found it: each run of its
/// words as it lies in a bitmap -- the whole of it in one chunk, or a
/// chunk a run -- with where the run goes in a mask's words. None of a
/// run's words, where its bitmap is not hot.
fn tile_word_runs<'t>(tick_state: &'t SimulationTickState, layer_type: LayerType, tile_first_cell: CellIndex, tile_side: u32) -> impl Iterator<Item = (Range<usize>, Option<&'t [u64]>)> {
    let part = tile_side.min(CHUNK_SIDE as u32);
    let (words, chunks) = ((part * part / WORD) as usize, (tile_side / part) * (tile_side / part));
    // A chunk's cells are 16 bits of a cell's index: the chunks of a tile follow one another above them.
    (0..chunks as usize).map(move |chunk| (chunk * words..(chunk + 1) * words, tick_state.aligned_tile(layer_type, CellIndex(tile_first_cell.0 + ((chunk as u64) << (2 * CHUNK_SIDE.trailing_zeros()))), part)))
}

/// Reads the aligned tile of `layer_type` that `cell_in_tile` is in, as the tick
/// found it, into `set_mask` -- the cells it holds at -- and `hot` -- those
/// in hot bitmaps: in the world, and read. Both of one side, the
/// tile's: its words copied as they lie.
pub fn read_layer(tick_state: &SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, set_mask: &mut Mask, hot: &mut Mask) {
    read_where(tick_state, layer_type, cell_in_tile, None, set_mask, hot);
}

/// [`read_layer`], of the cells set in `under_mask` alone: the rest are left
/// clear in `set_mask` and `hot`.
pub fn layer_under(tick_state: &SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, under_mask: &Mask, set_mask: &mut Mask, hot: &mut Mask) {
    read_where(tick_state, layer_type, cell_in_tile, Some(under_mask), set_mask, hot);
}

/// [`read_layer`], under `under_mask` if there is one.
fn read_where(tick_state: &SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, under_mask: Option<&Mask>, set_mask: &mut Mask, hot: &mut Mask) {
    let tile_side = set_mask.side_in_cells;
    assert!(hot.side_in_cells == tile_side && under_mask.is_none_or(|under_checked| under_checked.side_in_cells == tile_side), "masks of two sides");
    let tile_first_cell = tile(cell_in_tile, tile_side);
    if tile_side == SMALL_TILE_SIDE {
        let [tiles] = tick_state.small_tiles([layer_type], tile_first_cell, under_mask.map_or(SMALL_TILE, |under_words| under_words.words[0]));
        (set_mask.words[0], hot.words[0]) = (tiles.cells_set, tiles.hot);
        return;
    }
    for (word_run, words) in tile_word_runs(tick_state, layer_type, tile_first_cell, tile_side) {
        match words {
            Some(words) => {
                set_mask.words[word_run.clone()].copy_from_slice(words);
                hot.words[word_run].fill(u64::MAX);
            }
            None => {
                set_mask.words[word_run.clone()].fill(0);
                hot.words[word_run].fill(0);
            }
        }
    }
    if let Some(under_given) = under_mask {
        set_mask.and_with(under_given);
        hot.and_with(under_given);
    }
}

/// Whether `layer_type` holds, as the tick found it, at any cell set
/// in `mask`, the aligned tile `cell_in_tile` is in: the tile's words ANDed with
/// the mask's as they lie, and asked whether anything is left. Nothing
/// is copied.
pub fn any_under(tick_state: &SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, mask: &Mask) -> bool {
    let tile_first_cell = tile(cell_in_tile, mask.side_in_cells);
    if mask.side_in_cells == SMALL_TILE_SIDE {
        let [tiles] = tick_state.small_tiles([layer_type], tile_first_cell, mask.words[0]);
        return tiles.cells_set != 0;
    }
    tile_word_runs(tick_state, layer_type, tile_first_cell, mask.side_in_cells).any(|(word_run, words)| words.is_some_and(|words| words.iter().zip(&mask.words[word_run]).any(|(tile_word, mask_word)| tile_word & mask_word != 0)))
}

/// Queues `layer_type` holding at every cell set in `mask`, the
/// aligned tile `cell_in_tile` is in, that the rule sees clear: how many writes,
/// a compare-and-write each.
pub fn set_under(tick_state: &mut SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, mask: &Mask) -> usize {
    write_under(tick_state, layer_type, cell_in_tile, mask, true)
}

/// Queues `layer_type` no longer holding at any cell set in `mask`,
/// the aligned tile `cell_in_tile` is in, that the rule sees set: how many
/// writes, a compare-and-write each.
pub fn clear_under(tick_state: &mut SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, mask: &Mask) -> usize {
    write_under(tick_state, layer_type, cell_in_tile, mask, false)
}

/// Queues every hot cell set in `mask` that does not hold `layer_type`
/// as `becomes_holding` says becoming so, each a compare-and-write of what the
/// rule saw there -- no cell written over another's write. How many.
/// A set bit is its cell: its index added to the tile's first cell's.
fn write_under(tick_state: &mut SimulationTickState, layer_type: LayerType, cell_in_tile: CellIndex, mask: &Mask, becomes_holding: bool) -> usize {
    let tile_first_cell = tile(cell_in_tile, mask.side_in_cells);
    let mut writes_queued = 0;
    for cell_place in mask.set_bits() {
        let written_cell = CellIndex(tile_first_cell.0 | cell_place as u64);
        // A cell not hot is not written.
        if tick_state.layer_holds(layer_type, written_cell) == Ok(!becomes_holding) {
            crate::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, layer_type, written_cell, u32::from(!becomes_holding), u32::from(becomes_holding), None);
            writes_queued += 1;
        }
    }
    writes_queued
}
