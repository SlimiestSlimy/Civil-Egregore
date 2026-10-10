//! Masks: a square of cells as bits, [`SIDES`] a side, a row a run of
//! words -- cell `(x, y)` from its top left at bit `x` of row `y` -- a
//! square of a layer read into them, whole or under another, and a
//! layer written under one (`docs/instructions.md`, "Masks").

use crate::around::set_bit_of_rank;
use bitplane_manager::{Shape, Write, WriteOp};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;
use utilities::rng::Rng;

/// The sides a mask comes in: powers of two from 4 to an entity's
/// reach.
pub const SIDES: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// Bits in a word of a mask's row.
const WORD: u32 = u64::BITS;
/// Cells along the side of a window, the most a turn reads at once.
const WINDOW: u32 = 8;
/// The most cells along a rectangle write's side.
const RECT: u32 = u8::MAX as u32;

/// A square of cells, a bit each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// Cells along its side: one of [`SIDES`].
    side: u32,
    /// Its rows, top to bottom, each [`Mask::row_words`] words, a cell's
    /// bit clear past the side.
    words: Vec<u64>,
}

impl Mask {
    /// A mask `side` cells a side, no cell set.
    pub fn empty(side: u32) -> Self {
        assert!(SIDES.contains(&side), "a mask {side} cells a side: not one of {SIDES:?}");
        Self { side, words: vec![0; (side.div_ceil(WORD) * side) as usize] }
    }

    /// A mask `side` cells a side, every cell set.
    pub fn full(side: u32) -> Self {
        let mut mask = Self::empty(side);
        let row = mask.row_words();
        for (at, word) in mask.words.iter_mut().enumerate() {
            let before = (at % row) as u32 * WORD;
            *word = if side - before >= WORD { u64::MAX } else { (1 << (side - before)) - 1 };
        }
        mask
    }

    /// A mask `side` cells a side, the cells set whose centres are no
    /// farther than half the side from the square's: the disc that
    /// fills it.
    pub fn disc(side: u32) -> Self {
        let mut mask = Self::empty(side);
        // In half cells, from the square's centre to a cell's.
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

    /// Words in a row.
    fn row_words(&self) -> usize {
        self.side.div_ceil(WORD) as usize
    }

    /// Row `y`, its cells from the left a bit each.
    pub fn row(&self, y: u32) -> &[u64] {
        let row = self.row_words();
        &self.words[y as usize * row..][..row]
    }

    /// Whether the cell `(x, y)` is set.
    pub fn get(&self, x: u32, y: u32) -> bool {
        self.row(y)[(x / WORD) as usize] >> (x % WORD) & 1 == 1
    }

    /// Makes the cell `(x, y)` set, or clear.
    pub fn set(&mut self, x: u32, y: u32, to: bool) {
        debug_assert!(x < self.side && y < self.side, "({x}, {y}) of a mask {} a side", self.side);
        let word = &mut self.words[y as usize * self.side.div_ceil(WORD) as usize + (x / WORD) as usize];
        *word = *word & !(1 << (x % WORD)) | (to as u64) << (x % WORD);
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

    /// The set cells, `(x, y)`, row by row.
    pub fn cells(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        let row = self.row_words();
        self.words.iter().enumerate().flat_map(move |(at, &word)| {
            let (x, y) = ((at % row) as u32 * WORD, (at / row) as u32);
            std::iter::successors((word != 0).then_some(word), |&left| Some(left & (left - 1)).filter(|&left| left != 0)).map(move |left| (x + left.trailing_zeros(), y))
        })
    }

    /// One of the set cells, `(x, y)`, drawn from `random`: none if
    /// none is set, and then nothing is drawn.
    pub fn pick(&self, random: &mut Rng) -> Option<(u32, u32)> {
        let count = self.count();
        if count == 0 {
            return None;
        }
        let mut rank = random.below(count as u64) as u32;
        let row = self.row_words();
        for (at, &word) in self.words.iter().enumerate() {
            if rank >= word.count_ones() {
                rank -= word.count_ones();
                continue;
            }
            return Some(((at % row) as u32 * WORD + set_bit_of_rank(word, rank), (at / row) as u32));
        }
        None
    }
}

/// The top left cell of the square `side` cells a side about `centre`
/// -- it `side / 2` across and down: none past the world's edge.
pub fn about(centre: CellIndex, side: u32) -> Option<CellIndex> {
    let reach = side as i32 / 2;
    centre.offset(-reach, -reach)
}

/// The cell `(x, y)` of the square whose top left cell is `origin`:
/// none past the world's edge.
pub fn cell(origin: CellIndex, x: u32, y: u32) -> Option<CellIndex> {
    origin.offset(x as i32, y as i32)
}

/// Reads the square of `layer_type` whose top left cell is `origin`,
/// as the tick found it, into `set` -- the cells it holds at -- and
/// `hot` -- those in hot bitmaps: in the world, and read. Both of one
/// side, the square's.
pub fn layer(turn: &Turn, layer_type: LayerType, origin: CellIndex, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, origin, None, set, hot);
}

/// [`layer`], of the cells set in `under` alone: the rest are left
/// clear in `set` and `hot`, and the parts of the square `under` has no
/// cell in are not read at all.
pub fn layer_under(turn: &Turn, layer_type: LayerType, origin: CellIndex, under: &Mask, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, origin, Some(under), set, hot);
}

/// [`layer`], under `under` if there is one: a window a time.
fn read_where(turn: &Turn, layer_type: LayerType, origin: CellIndex, under: Option<&Mask>, set: &mut Mask, hot: &mut Mask) {
    let side = set.side;
    assert!(hot.side == side && under.is_none_or(|under| under.side == side), "masks of two sides");
    set.clear();
    hot.clear();
    let (row, across) = (set.row_words(), WINDOW.min(side));
    for (x, y) in (0..side).step_by(WINDOW as usize).flat_map(|y| (0..side).step_by(WINDOW as usize).map(move |x| (x, y))) {
        let (word, shift) = ((x / WORD) as usize, x % WORD);
        let rows = (y..(y + WINDOW).min(side)).map(|y| y as usize * row + word);
        // The window's cells under the mask, as a window lays them out: a row a byte.
        let wanted = under.map_or(u64::MAX, |under| rows.clone().enumerate().fold(0, |wanted, (down, at)| wanted | (under.words[at] >> shift & 0xff) << (8 * down)));
        if wanted == 0 {
            continue;
        }
        // A window past the world's edge is left clear, and not hot.
        let Some(corner) = origin.offset(x as i32, y as i32) else {
            continue;
        };
        let window = turn.window(layer_type, corner, across, across);
        for (down, at) in rows.enumerate() {
            set.words[at] |= ((window.set & wanted) >> (8 * down) & 0xff) << shift;
            hot.words[at] |= ((window.hot & wanted) >> (8 * down) & 0xff) << shift;
        }
    }
}

/// Queues `layer_type` holding at every cell set in `mask`, the square
/// whose top left cell is `origin`: how many writes it took.
pub fn set_under(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask) -> usize {
    write_under(turn, layer_type, origin, mask, WriteOp::Set)
}

/// Queues `layer_type` no longer holding at any cell set in `mask`,
/// the square whose top left cell is `origin`: how many writes it took.
pub fn clear_under(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask) -> usize {
    write_under(turn, layer_type, origin, mask, WriteOp::Unset)
}

/// Queues `op` on every cell set in `mask`, as rectangles: each row's
/// runs of set cells, a run the same in the rows under it one
/// rectangle with them. How many writes.
fn write_under(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask, op: WriteOp) -> usize {
    // The rectangles still growing down: where each starts, and its sides.
    let mut open: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut queued = 0;
    let mut queue = |turn: &mut Turn, (x, y, width, height): (u32, u32, u32, u32)| {
        // A rectangle starting past the world's edge has no cell in it.
        if let Some(at) = origin.offset(x as i32, y as i32) {
            turn.queue(layer_type, Write { at, op, shape: Shape::Rect { width: width as u8, height: height as u8 } });
            queued += 1;
        }
    };
    for y in 0..mask.side {
        runs.clear();
        let mut x = 0;
        while let Some(start) = next_cell(mask, y, x, true) {
            let end = next_cell(mask, y, start, false).unwrap_or(mask.side).min(start + RECT);
            runs.push((start, end - start));
            x = end;
        }
        // A rectangle whose run is not this row's, or as tall as one gets, is done.
        open.retain_mut(|rect| {
            let grows = rect.3 < RECT && runs.iter().any(|&(start, width)| (start, width) == (rect.0, rect.2));
            if grows {
                rect.3 += 1;
            } else {
                queue(turn, *rect);
            }
            grows
        });
        for &(start, width) in &runs {
            if !open.iter().any(|rect| (rect.0, rect.2) == (start, width) && rect.1 + rect.3 > y) {
                open.push((start, y, width, 1));
            }
        }
    }
    open.into_iter().for_each(|rect| queue(turn, rect));
    queued
}

/// The first cell of row `y` of `mask`, from `x` on, that is set --
/// or, `set` false, that is clear: none if the row has none.
fn next_cell(mask: &Mask, y: u32, x: u32, set: bool) -> Option<u32> {
    let row = mask.row(y);
    let mut at = x;
    while at < mask.side {
        let word = if set { row[(at / WORD) as usize] } else { !row[(at / WORD) as usize] } >> (at % WORD);
        if word != 0 {
            return Some(at + word.trailing_zeros()).filter(|&found| found < mask.side);
        }
        at = (at / WORD + 1) * WORD;
    }
    None
}
