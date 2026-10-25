//! Render rules as the client runs them ([`RenderCode`]): registered
//! with the rule's type, run on a superchunk's turn once its cells
//! arrive ([`RenderTurn`]). As the server's rules: the rule's code is
//! for one drawn cell or one entity, and the client does the looping
//! ([`on_each_set_cell`], [`on_each_drawn_cell`], [`on_each_entity`]),
//! the rule's code inlined in it. A drawn cell is a square of cells
//! `2^detail_level` a side, the level matched to the zoom: the cells a
//! superchunk paints are about the pixels it takes on the screen,
//! however far out it is seen. What a rule paints is its render
//! layer's picture of the superchunk, Bevy's colours a drawn cell.

use bevy::color::{Color, ColorToPacked, LinearRgba};
use protocol::pipe::frame::{value_at, Cells, CHUNK_WORDS, WORD_BITS};
use protocol::type_registry::{Drawn, EntityType, PlaneType};
use std::sync::Arc;
use utilities::coordinates::SUPERCHUNK_SIDE_CELLS;

/// Cells along a superchunk's side, as a power of two: the coarsest
/// detail level, a superchunk one drawn cell.
pub const SUPERCHUNK_SIDE_LOG2: u32 = SUPERCHUNK_SIDE_CELLS.trailing_zeros();

/// Cells a chunk holds, as a power of two: a wide plane's chunk is so
/// many cells' bits, one cell's after another.
const CHUNK_CELLS_LOG2: u32 = (CHUNK_WORDS * WORD_BITS).trailing_zeros();

/// What a render rule runs with: what it draws.
#[derive(Clone, Copy, Debug)]
pub struct RenderContext {
    /// Its master: the plane or the type of entity it draws.
    pub master: Drawn,
}

/// A render rule on one superchunk's turn, with what it runs with: made
/// as its type is registered, with the IDs of the types it reads.
pub type RenderRun = dyn Fn(&mut RenderTurn, &RenderContext) + Send + Sync;

/// What runs a render rule: its library, the client's.
#[derive(Clone)]
pub struct RenderCode {
    /// The rule, on one superchunk's turn.
    pub run: Arc<RenderRun>,
}

impl RenderCode {
    /// The rule `run`.
    pub fn new(run: impl Fn(&mut RenderTurn, &RenderContext) + Send + Sync + 'static) -> Self {
        Self { run: Arc::new(run) }
    }
}

impl std::fmt::Debug for RenderCode {
    /// Nothing of it: what the rule is, only its code knows.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.debug_struct("RenderCode").finish_non_exhaustive()
    }
}

/// One drawn cell of a superchunk: a square of its cells
/// `2^detail_level` a side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawnCell {
    /// Its place among the superchunk's drawn cells, in Morton order:
    /// its cells are the superchunk's from `morton_index << 2 * detail_level` on.
    pub morton_index: usize,
}

impl DrawnCell {
    /// The drawn cell at `(x, y)` among the superchunk's drawn cells.
    pub fn at(x: u32, y: u32) -> Self {
        Self { morton_index: (spread_bits(x) | spread_bits(y) << 1) as usize }
    }

    /// Where it is among the superchunk's drawn cells, `(x, y)` from its
    /// top left.
    pub fn cartesian(self) -> (u32, u32) {
        (gather_even_bits(self.morton_index as u64), gather_even_bits(self.morton_index as u64 >> 1))
    }
}

/// `coordinate`'s bits spread to every other bit: bit `i` to bit `2i`.
const fn spread_bits(coordinate: u32) -> u64 {
    let mut spread = coordinate as u64 & 0xffff_ffff;
    spread = (spread | spread << 16) & 0x0000_ffff_0000_ffff;
    spread = (spread | spread << 8) & 0x00ff_00ff_00ff_00ff;
    spread = (spread | spread << 4) & 0x0f0f_0f0f_0f0f_0f0f;
    spread = (spread | spread << 2) & 0x3333_3333_3333_3333;
    (spread | spread << 1) & 0x5555_5555_5555_5555
}

/// The even bits of `morton_index` gathered: one coordinate of it.
const fn gather_even_bits(morton_index: u64) -> u32 {
    let mut gathered = morton_index & 0x5555_5555_5555_5555;
    gathered = (gathered | gathered >> 1) & 0x3333_3333_3333_3333;
    gathered = (gathered | gathered >> 2) & 0x0f0f_0f0f_0f0f_0f0f;
    gathered = (gathered | gathered >> 4) & 0x00ff_00ff_00ff_00ff;
    gathered = (gathered | gathered >> 8) & 0x0000_ffff_0000_ffff;
    ((gathered | gathered >> 16) & 0xffff_ffff) as u32
}

/// One superchunk's turn of the render rules of one render layer: its
/// cells as the frame brought them, read at its detail level, and the
/// layer's picture of it, painted a drawn cell at a time.
pub struct RenderTurn<'turn> {
    /// The superchunk's cells.
    cells: &'turn Cells,
    /// Its drawn cells are `2^detail_level` cells a side.
    detail_level: u32,
    /// The render layer's picture of it: RGBA, a drawn cell a pixel, row by row.
    picture: &'turn mut [u8],
    /// Whether anything was painted.
    pub(crate) painted: bool,
}

impl<'turn> RenderTurn<'turn> {
    /// A turn on `cells` at `detail_level`, painting into `picture`.
    pub(crate) fn new(cells: &'turn Cells, detail_level: u32, picture: &'turn mut [u8]) -> Self {
        debug_assert_eq!(picture.len(), 4 * (drawn_side(detail_level) as usize).pow(2), "a picture a drawn cell a pixel");
        Self { cells, detail_level, picture, painted: false }
    }

    /// Its drawn cells are `2^detail_level` cells a side.
    pub fn detail_level(&self) -> u32 {
        self.detail_level
    }

    /// Drawn cells along the superchunk's side.
    pub fn drawn_side(&self) -> u32 {
        drawn_side(self.detail_level)
    }

    /// The superchunk's cells as the frame brought them.
    pub fn cells(&self) -> &'turn Cells {
        self.cells
    }

    /// The bits of `drawn`'s cells in a plane of a bit a cell: the first, and how many.
    fn bits_of(&self, drawn: DrawnCell) -> (usize, usize) {
        let block_log2 = 2 * self.detail_level;
        (drawn.morton_index << block_log2, 1 << block_log2)
    }

    /// How many of `drawn`'s cells `layer` has set, of how many it has.
    pub fn count_set(&self, layer: PlaneType, drawn: DrawnCell) -> (u32, u32) {
        let (first_bit, bit_count) = self.bits_of(drawn);
        let words = self.cells.plane(layer);
        if words.is_empty() {
            return (0, bit_count as u32);
        }
        if bit_count < WORD_BITS {
            let mask = (u64::MAX >> (WORD_BITS - bit_count)) << (first_bit % WORD_BITS);
            return ((words[first_bit / WORD_BITS] & mask).count_ones(), bit_count as u32);
        }
        let first_word = first_bit / WORD_BITS;
        (words[first_word..first_word + bit_count / WORD_BITS].iter().map(|word| word.count_ones()).sum(), bit_count as u32)
    }

    /// The share of `drawn`'s cells `layer` has set, from 0 to 1.
    pub fn share_set(&self, layer: PlaneType, drawn: DrawnCell) -> f32 {
        let (set_cells, all_cells) = self.count_set(layer, drawn);
        set_cells as f32 / all_cells as f32
    }

    /// The number `plane` holds at `drawn`'s first cell
    /// -- its tile's, of a plane of tiles: what stands for them all.
    pub fn value(&self, plane: PlaneType, drawn: DrawnCell) -> u32 {
        let first_cell = drawn.morton_index << (2 * self.detail_level);
        let chunk_first_word = (first_cell >> CHUNK_CELLS_LOG2) * plane.chunk_words();
        let words = self.cells.plane(plane).get(chunk_first_word..).unwrap_or_default();
        value_at(words, plane, (first_cell & ((1 << CHUNK_CELLS_LOG2) - 1)) >> plane.tile_size().shift())
    }

    /// Paints `drawn` `color`, over what the layer's rules painted before.
    fn paint(&mut self, drawn: DrawnCell, color: Color) {
        let (x, y) = drawn.cartesian();
        let first_byte = 4 * (y * self.drawn_side() + x) as usize;
        self.picture[first_byte..first_byte + 4].copy_from_slice(&color.to_srgba().to_u8_array());
        self.painted = true;
    }
}

/// Drawn cells along a superchunk's side at `detail_level`.
pub(crate) const fn drawn_side(detail_level: u32) -> u32 {
    SUPERCHUNK_SIDE_CELLS >> detail_level
}

/// One superchunk's turn of a render rule of a layer of a bit a cell:
/// `rule`, the function on one drawn cell, run by the client on each
/// drawn cell any of whose cells `layer` has set, painting it the colour
/// it gives, if any.
#[inline]
pub fn on_each_set_cell(turn: &mut RenderTurn, layer: PlaneType, mut rule: impl FnMut(&RenderTurn, DrawnCell) -> Option<Color>) {
    let words = turn.cells.plane(layer);
    let block_log2 = 2 * turn.detail_level;
    let (mut next_bit, all_bits) = (0, words.len() * WORD_BITS);
    while next_bit < all_bits {
        let word_index = next_bit / WORD_BITS;
        let word = words[word_index] & (u64::MAX << (next_bit % WORD_BITS));
        if word == 0 {
            next_bit = (word_index + 1) * WORD_BITS;
            continue;
        }
        let drawn = DrawnCell { morton_index: (word_index * WORD_BITS + word.trailing_zeros() as usize) >> block_log2 };
        if let Some(color) = rule(turn, drawn) {
            turn.paint(drawn, color);
        }
        // Its cells are one run of bits: the next drawn cell's start after them.
        next_bit = (drawn.morton_index + 1) << block_log2;
    }
}

/// One superchunk's turn of a render rule drawing every drawn cell --
/// of a wide plane, say: `rule` run on each, painting it the colour it
/// gives, if any.
#[inline]
pub fn on_each_drawn_cell(turn: &mut RenderTurn, mut rule: impl FnMut(&RenderTurn, DrawnCell) -> Option<Color>) {
    for morton_index in 0..(turn.drawn_side() as usize).pow(2) {
        let drawn = DrawnCell { morton_index };
        if let Some(color) = rule(turn, drawn) {
            turn.paint(drawn, color);
        }
    }
}

/// One superchunk's turn of a render rule of a type of entity: `rule`
/// run on each entity of `entity_type` in it, with the drawn cell it
/// stands in, giving its colour, if any. An entity covers its cell: a
/// drawn cell of many is painted their entities' colours mixed, as
/// opaque as the share of its cells they stand on.
#[inline]
pub fn on_each_entity(turn: &mut RenderTurn, entity_type: EntityType, mut rule: impl FnMut(&RenderTurn, DrawnCell) -> Option<Color>) {
    let (cells, detail_level) = (turn.cells, turn.detail_level);
    let mut coloured: Vec<(DrawnCell, Color)> = cells
        .entities
        .iter()
        .filter(|(entity_of, _)| *entity_of == entity_type)
        .filter_map(|&(_, (x, y))| {
            let drawn = DrawnCell::at(u32::from(x) >> detail_level, u32::from(y) >> detail_level);
            rule(turn, drawn).map(|color| (drawn, color))
        })
        .collect();
    if detail_level == 0 {
        coloured.into_iter().for_each(|(drawn, color)| turn.paint(drawn, color));
        return;
    }
    coloured.sort_unstable_by_key(|(drawn, _)| drawn.morton_index);
    let cells_in_drawn = (1u32 << (2 * detail_level)) as f32;
    for same_drawn in coloured.chunk_by(|(drawn, _), (other_drawn, _)| drawn == other_drawn) {
        let mixed = same_drawn.iter().fold(LinearRgba::NONE, |mixed, (_, color)| mixed + color.to_linear());
        let entities_in = same_drawn.len() as f32;
        turn.paint(same_drawn[0].0, Color::LinearRgba(LinearRgba { alpha: mixed.alpha / cells_in_drawn, ..mixed / entities_in }));
    }
}
