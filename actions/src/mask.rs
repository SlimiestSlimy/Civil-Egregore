//! Masks: an aligned tile's cells as bits, [`SIDES`] a side, in
//! Morton order -- as the tile lies in its bitmap, one run of bits --
//! so a layer is read into one by copying words, asked under one with
//! an AND, and written under one a set bit a cell
//! (`docs/actions.md`, "Masks").

use crate::around::set_bit_of_rank;
use bitplane_manager::{SMALL_TILE_CELLS, SMALL_TILE_SIDE};
use chunk_storage::LayerType;
use coordinates::{cartesian_from_place, place_from_cartesian, CellIndex, CHUNK_SIDE};
use simulation::Turn;
use std::ops::Range;
use utilities::rng::Rng;

/// The sides a mask comes in: powers of two from a small tile's 4 to
/// a superchunk's, an entity's reach.
pub const SIDES: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// Bits in a word of a mask.
const WORD: u32 = u64::BITS;
/// The bits of a small tile, the one mask that is less than a word.
const SMALL_TILE: u64 = (1 << SMALL_TILE_CELLS) - 1;

/// An aligned tile of cells, a bit each: the cell `(x, y)` from the
/// tile's top left at the bit whose index is the two interleaved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// Cells along its side: one of [`SIDES`].
    side: u32,
    /// Its cells in Morton order, a cell's bit clear past the tile's --
    /// which only a small tile's word has any past.
    words: Vec<u64>,
}

impl Mask {
    /// A mask `side` cells a side, no cell set.
    pub fn empty(side: u32) -> Self {
        assert!(SIDES.contains(&side), "a mask {side} cells a side: not one of {SIDES:?}");
        Self { side, words: vec![0; (side * side).div_ceil(WORD) as usize] }
    }

    /// A mask `side` cells a side, every cell set.
    pub fn full(side: u32) -> Self {
        let mut mask = Self::empty(side);
        mask.words.fill(if side == SMALL_TILE_SIDE { SMALL_TILE } else { u64::MAX });
        mask
    }

    /// A mask `side` cells a side, the cells set whose centres are no
    /// farther than half the side from the tile's: the disc that
    /// fills it.
    pub fn disc(side: u32) -> Self {
        let mut mask = Self::empty(side);
        // In half cells, from the tile's centre to a cell's.
        let reach = side as i64 * side as i64;
        for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
            let (dx, dy) = (2 * x as i64 + 1 - side as i64, 2 * y as i64 + 1 - side as i64);
            if dx * dx + dy * dy <= reach {
                mask.set(x, y, true);
            }
        }
        mask
    }

    /// Cells along its side.
    pub fn side(&self) -> u32 {
        self.side
    }

    /// Its cells in Morton order, a word 8x8 of them: as the tile's
    /// lie in a bitmap.
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// Whether the cell `(x, y)` is set.
    pub fn get(&self, x: u32, y: u32) -> bool {
        let bit = place_from_cartesian(x, y);
        self.words[bit / WORD as usize] >> (bit % WORD as usize) & 1 == 1
    }

    /// Makes the cell `(x, y)` set, or clear.
    pub fn set(&mut self, x: u32, y: u32, to: bool) {
        debug_assert!(x < self.side && y < self.side, "({x}, {y}) of a mask {} a side", self.side);
        let bit = place_from_cartesian(x, y);
        let word = &mut self.words[bit / WORD as usize];
        *word = *word & !(1 << (bit % WORD as usize)) | (to as u64) << (bit % WORD as usize);
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

    /// Keeps the cells `other`, of the same side, has set too.
    pub fn and(&mut self, other: &Mask) {
        self.with(other, |word, other| word & other);
    }

    /// Sets the cells `other`, of the same side, has set.
    pub fn or(&mut self, other: &Mask) {
        self.with(other, |word, other| word | other);
    }

    /// Clears the cells `other`, of the same side, has set.
    pub fn and_not(&mut self, other: &Mask) {
        self.with(other, |word, other| word & !other);
    }

    /// Each word made `of` it and `other`'s.
    fn with(&mut self, other: &Mask, of: impl Fn(u64, u64) -> u64) {
        assert_eq!(self.side, other.side, "masks of two sides");
        for (word, &other) in self.words.iter_mut().zip(&other.words) {
            *word = of(*word, other);
        }
    }

    /// The set cells' bits, in Morton order: each its cell's place in
    /// the tile.
    fn bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(move |(at, &word)| {
            std::iter::successors((word != 0).then_some(word), |&left| Some(left & (left - 1)).filter(|&left| left != 0)).map(move |left| at * WORD as usize + left.trailing_zeros() as usize)
        })
    }

    /// The set cells, `(x, y)`, in Morton order.
    pub fn cells(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.bits().map(cartesian_from_place)
    }

    /// One of the set cells, `(x, y)`, drawn from `random`: none if
    /// none is set, and then nothing is drawn.
    pub fn pick(&self, random: &mut Rng) -> Option<(u32, u32)> {
        let count = self.count();
        if count == 0 {
            return None;
        }
        let mut rank = random.below(count as u64) as u32;
        for (at, &word) in self.words.iter().enumerate() {
            if rank >= word.count_ones() {
                rank -= word.count_ones();
                continue;
            }
            return Some(cartesian_from_place(at * WORD as usize + set_bit_of_rank(word, rank) as usize));
        }
        None
    }
}

/// The first cell -- the top left one -- of the aligned tile `side`
/// cells a side that `at` is in: the tile a mask of that side stands
/// for there.
pub fn tile(at: CellIndex, side: u32) -> CellIndex {
    CellIndex(at.0 & !(u64::from(side) * u64::from(side) - 1))
}

/// The cell `(x, y)` of the tile whose first cell is `first`.
pub fn cell(first: CellIndex, x: u32, y: u32) -> CellIndex {
    CellIndex(first.0 | place_from_cartesian(x, y) as u64)
}

/// The tile `side` cells a side (8 at least) whose first cell is
/// `first`, of `layer_type`, as the tick found it: each run of its
/// words as it lies in a bitmap -- the whole of it in one chunk, or a
/// chunk a run -- with where the run goes in a mask's words. None of a
/// run's words, where its bitmap is not hot.
fn runs<'t>(turn: &'t Turn, layer_type: LayerType, first: CellIndex, side: u32) -> impl Iterator<Item = (Range<usize>, Option<&'t [u64]>)> {
    let part = side.min(CHUNK_SIDE as u32);
    let (words, chunks) = ((part * part / WORD) as usize, (side / part) * (side / part));
    // A chunk's cells are 16 bits of a cell's index: the chunks of a tile follow one another above them.
    (0..chunks as usize).map(move |chunk| (chunk * words..(chunk + 1) * words, turn.aligned_tile(layer_type, CellIndex(first.0 + ((chunk as u64) << (2 * CHUNK_SIDE.trailing_zeros()))), part)))
}

/// Reads the aligned tile of `layer_type` that `at` is in, as the tick
/// found it, into `set` -- the cells it holds at -- and `hot` -- those
/// in hot bitmaps: in the world, and read. Both of one side, the
/// tile's: its words copied as they lie.
pub fn layer(turn: &Turn, layer_type: LayerType, at: CellIndex, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, at, None, set, hot);
}

/// [`layer`], of the cells set in `under` alone: the rest are left
/// clear in `set` and `hot`.
pub fn layer_under(turn: &Turn, layer_type: LayerType, at: CellIndex, under: &Mask, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, at, Some(under), set, hot);
}

/// [`layer`], under `under` if there is one.
fn read_where(turn: &Turn, layer_type: LayerType, at: CellIndex, under: Option<&Mask>, set: &mut Mask, hot: &mut Mask) {
    let side = set.side;
    assert!(hot.side == side && under.is_none_or(|under| under.side == side), "masks of two sides");
    let first = tile(at, side);
    if side == SMALL_TILE_SIDE {
        let [tiles] = turn.small_tiles([layer_type], first, under.map_or(SMALL_TILE, |under| under.words[0]));
        (set.words[0], hot.words[0]) = (tiles.set, tiles.hot);
        return;
    }
    for (run, words) in runs(turn, layer_type, first, side) {
        match words {
            Some(words) => {
                set.words[run.clone()].copy_from_slice(words);
                hot.words[run].fill(u64::MAX);
            }
            None => {
                set.words[run.clone()].fill(0);
                hot.words[run].fill(0);
            }
        }
    }
    if let Some(under) = under {
        set.and(under);
        hot.and(under);
    }
}

/// Whether `layer_type` holds, as the tick found it, at any cell set
/// in `mask`, the aligned tile `at` is in: the tile's words ANDed with
/// the mask's as they lie, and asked whether anything is left. Nothing
/// is copied.
pub fn any_under(turn: &Turn, layer_type: LayerType, at: CellIndex, mask: &Mask) -> bool {
    let first = tile(at, mask.side);
    if mask.side == SMALL_TILE_SIDE {
        let [tiles] = turn.small_tiles([layer_type], first, mask.words[0]);
        return tiles.set != 0;
    }
    runs(turn, layer_type, first, mask.side).any(|(run, words)| words.is_some_and(|words| words.iter().zip(&mask.words[run]).any(|(held, wanted)| held & wanted != 0)))
}

/// Queues `layer_type` holding at every cell set in `mask`, the
/// aligned tile `at` is in, that the rule sees clear: how many writes,
/// a compare-and-write each.
pub fn set_under(turn: &mut Turn, layer_type: LayerType, at: CellIndex, mask: &Mask) -> usize {
    write_under(turn, layer_type, at, mask, true)
}

/// Queues `layer_type` no longer holding at any cell set in `mask`,
/// the aligned tile `at` is in, that the rule sees set: how many
/// writes, a compare-and-write each.
pub fn clear_under(turn: &mut Turn, layer_type: LayerType, at: CellIndex, mask: &Mask) -> usize {
    write_under(turn, layer_type, at, mask, false)
}

/// Queues every hot cell set in `mask` that does not hold `layer_type`
/// as `holding` says becoming so, each a compare-and-write of what the
/// rule saw there -- no cell written over another's write. How many.
/// A set bit is its cell: its index added to the tile's first cell's.
fn write_under(turn: &mut Turn, layer_type: LayerType, at: CellIndex, mask: &Mask, holding: bool) -> usize {
    let first = tile(at, mask.side);
    let mut queued = 0;
    for bit in mask.bits() {
        let at = CellIndex(first.0 | bit as u64);
        // A cell not hot is not written.
        if turn.holds(layer_type, at) == Ok(!holding) {
            crate::cells::seen_becoming(turn, layer_type, at, u32::from(!holding), u32::from(holding), None);
            queued += 1;
        }
    }
    queued
}
