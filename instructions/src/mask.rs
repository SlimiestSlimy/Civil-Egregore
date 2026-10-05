//! Masks: a square of cells as bits, read from a layer at once and
//! written to one under it. A [`Mask`] is `side` by `side` cells --
//! [`SIDES`], 4 to 1,024, an entity's reach -- a row a run of words,
//! cell `(x, y)` from its top left at bit `x` of row `y`. Sets of cells
//! are masks put together with `&`, `|` and `!`: the cells of a layer
//! in a square (`read::mask`), those hot, those of a shape -- a disc,
//! say -- and what is to be set or cleared (`write::mask`).
//!
//! A mask is room kept by the rule and read into, never made a read:
//! the largest is 128 KiB.

use coordinates::CellIndex;
use utilities::rng::Rng;

/// The sides a mask comes in: powers of two from 4 to an entity's
/// reach.
pub const SIDES: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// Bits in a word of a mask's row.
pub(crate) const WORD: u32 = u64::BITS;
/// A square of cells, a bit each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// Cells along its side: one of [`SIDES`].
    pub(crate) side: u32,
    /// Its rows, top to bottom, each [`Mask::row_words`] words, a cell's
    /// bit clear past the side.
    pub(crate) words: Vec<u64>,
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
    pub(crate) fn row_words(&self) -> usize {
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
            let mut left = word;
            (0..rank).for_each(|_| left &= left - 1);
            return Some(((at % row) as u32 * WORD + left.trailing_zeros(), (at / row) as u32));
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
