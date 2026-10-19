//! What a client asks of the world's viewport ([`FrameAsk`]) and what it is
//! answered with ([`ViewportFrame`]): the hot superchunks asked for, each
//! copied as a tick left it and nothing more -- the planes asked for, by
//! their layer types, whatever they are: the server names none.

use crate::type_registry::LayerType;
use utilities::wire::{WireReader, Wired, WiredStruct};

/// Words a chunk's bitmap takes.
pub const CHUNK_WORDS: usize = utilities::bitmap::WORDS;

/// Bits a word of a bitmap holds.
pub const WORD_BITS: usize = utilities::bitmap::BITS_PER_WORD;

/// Where in its chunk, `(x, y)`, the cell is that the `bit_index`-th bit of
/// a chunk's bitmap stands for: the bits are in Morton order.
pub const fn cell_of_bit(bit_index: usize) -> (u8, u8) {
    utilities::bitmap::morton::morton_coordinates(bit_index)
}

/// What the renderer should render, in superchunks: a rectangle of
/// them, each `(x, y)` in superchunks from the world's top left, both
/// corners in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    /// The top left superchunk, `(x, y)`.
    pub top_left_superchunk_cartesian: (u32, u32),
    /// The bottom right one.
    pub bottom_right_superchunk_cartesian: (u32, u32),
}

impl WiredStruct for Viewport {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|viewport| &mut viewport.top_left_superchunk_cartesian, |viewport| &mut viewport.bottom_right_superchunk_cartesian];
}

impl Viewport {
    /// Whether `(x, y)`, in superchunks, is in it.
    pub fn contains(self, (x, y): (u32, u32)) -> bool {
        (self.top_left_superchunk_cartesian.0..=self.bottom_right_superchunk_cartesian.0).contains(&x) && (self.top_left_superchunk_cartesian.1..=self.bottom_right_superchunk_cartesian.1).contains(&y)
    }
}

/// What a client wants of the world, one frame: some of the hot
/// superchunks of its viewport -- as many as a frame may carry, the
/// client going round them frame after frame -- how finely it will
/// draw them, which of their planes, and which layers counted over the
/// whole world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameAsk {
    /// What the client renders, none if it renders none of the world's
    /// cells (its map, say, drawn from generation alone).
    pub viewport: Option<Viewport>,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    /// Passed on in the frame; the host copies the same.
    pub pixel_side_log2: u32,
    /// The viewport's hot superchunks, row by row, to pass over first.
    pub superchunks_skipped: u32,
    /// Superchunks to answer with, at most.
    pub most_superchunks: u32,
    /// The viewport's cells, if they are seen from near: drawn as one
    /// picture, a cell many pixels. Passed on in the frame.
    pub seen_near: Option<NearRectangle>,
    /// The planes of each superchunk sent ([`Cells::planes`]), in this order.
    pub planes: Vec<LayerType>,
    /// The layers whose cells are counted over every hot superchunk
    /// ([`ViewportFrame::counts`]), in this order.
    pub counted: Vec<LayerType>,
}

/// A layer type on the wire: its ID, then its shape -- bits a cell and
/// tiles' scale -- in a word.
impl Wired for LayerType {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.extend([self.type_id(), u64::from(self.width_bits()) << 32 | u64::from(self.tile_scale())]);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        let (mut type_id, mut shape) = (0u64, 0u64);
        type_id.take(wire)?;
        shape.take(wire)?;
        *self = LayerType::of_id_and_shape(type_id, (shape >> 32) as u32, shape as u32)?;
        Some(())
    }
}

/// Cells seen from near: a rectangle of them, each many pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NearRectangle {
    /// The top left cell, `(x, y)` in the world.
    pub first_cell_cartesian: (u32, u32),
    /// Cells across and down.
    pub size_in_cells: (u32, u32),
    /// Pixels along a cell's side: 2, 4 or 8.
    pub pixels_a_cell: u32,
}

/// One hot superchunk's cells, as a tick left them.
#[derive(Clone, Default)]
pub struct Cells {
    /// Where it is in the world, `(x, y)` in superchunks.
    pub superchunk_cartesian: (u32, u32),
    /// The planes asked for ([`FrameAsk::planes`]), each its layer type
    /// and its 16 chunks' buckets one after another, in the chunks'
    /// Morton order, as the arena holds the plane: a bitmap's
    /// [`CHUNK_WORDS`] words a chunk, in Morton order, so many times
    /// over as the plane has bits a cell. A chunk not hot is all clear;
    /// a plane all clear is sent empty ([`Cells::plane`]).
    pub planes: Vec<(LayerType, Vec<u64>)>,
    /// Its top left cell in the world, `(x, y)`: what its ground is
    /// worked out from.
    pub top_left: (u32, u32),
    /// The cells its entities stand on, `(x, y)` from its top left.
    pub entity_cells_cartesian: Vec<(u16, u16)>,
}

impl Cells {
    /// The words of its plane of `layer_type`: empty if it was not
    /// asked for, or is all clear.
    pub fn plane(&self, layer_type: LayerType) -> &[u64] {
        self.planes.iter().find(|(sent_type, _)| *sent_type == layer_type).map_or(&[], |(_, words)| words)
    }

    /// Its plane of `layer_type` taken out of it, empty as
    /// [`Cells::plane`] would be.
    pub fn take_plane(&mut self, layer_type: LayerType) -> Vec<u64> {
        self.planes.iter_mut().find(|(sent_type, _)| *sent_type == layer_type).map_or_else(Vec::new, |(_, words)| std::mem::take(words))
    }
}

/// Whether the `bit_index`-th bit of a bitmap's `words` is set: not
/// where none was sent.
pub fn bit_set(words: &[u64], bit_index: usize) -> bool {
    words.get(bit_index / WORD_BITS).is_some_and(|word| word >> (bit_index % WORD_BITS) & 1 == 1)
}

/// The number at the `place`-th cell of a wide plane's `words`, of
/// `width_bits` bits a cell, each cell's bits one after another: none
/// where none was sent.
pub fn value_at(words: &[u64], width_bits: u32, place: usize) -> u32 {
    let first_bit = place * width_bits as usize;
    words.get(first_bit / WORD_BITS).map_or(0, |word| (word >> (first_bit % WORD_BITS)) as u32 & (u32::MAX >> (u32::BITS - width_bits)))
}

/// The world's viewport, as a tick left it.
#[derive(Clone)]
pub struct ViewportFrame {
    /// Counts the worlds the host has run: what a client made of an
    /// earlier one is made again.
    pub world: u64,
    /// The seed of the world run.
    pub seed: u64,
    /// How it is generated: each of its generation's parameters, by its
    /// name, as a world's file keeps them.
    pub worldgen_parameters: Vec<(String, u64)>,
    /// The heights it is drawn between.
    pub drawn_heights: super::terrain::DrawnHeightRange,
    /// Superchunks along its side, a square about its origin, if it has
    /// a size.
    pub side_superchunks: Option<u32>,
    /// Ticks run so far.
    pub tick: u64,
    /// Ticks a second, over the time between the last two asks
    /// answered whole.
    pub ticks_a_second: f64,
    /// Ticks a second the server holds a world to unless told otherwise:
    /// a second of the simulation in a second -- the simulation's to say,
    /// a client's to pace by.
    pub target_pace: u32,
    /// Ticks a second the server holds the world to now: none, flat out.
    pub held_pace: Option<u32>,
    /// Entities in the whole world.
    pub entity_count: usize,
    /// The cells of each layer counted ([`FrameAsk::counted`]) over
    /// every hot superchunk, in the order asked.
    pub counts: Vec<(LayerType, u64)>,
    /// What answering the last ask answered whole took of the host's
    /// thread -- the copy, all a client costs it -- in seconds.
    pub sync_seconds: f64,
    /// The share of the thread's time that is, at the rate asked.
    pub sync_share: f64,
    /// What the client renders ([`FrameAsk::viewport`]).
    pub viewport: Option<Viewport>,
    /// Every hot superchunk of the viewport, row by row: the client
    /// goes round them, and whatever else of the viewport it drew is
    /// cold now.
    pub hot: Vec<(u32, u32)>,
    /// How coarsely the client will draw them ([`FrameAsk::pixel_side_log2`]).
    pub pixel_side_log2: u32,
    /// The cells seen from near ([`FrameAsk::seen_near`]).
    pub seen_near: Option<NearRectangle>,
    /// The name of the world run, if it is one of the worlds' folder:
    /// opened from it, or saved to it.
    pub world_name: Option<String>,
    /// What opening or saving a world last came to, if either was asked.
    pub last_outcome: Option<String>,
    /// Whether more of what was asked is to come, in frames after
    /// this one: an ask is answered a few superchunks at a time.
    pub more_to_come: bool,
    /// Superchunks asked for: those copied since the frame before.
    pub cells: Vec<Cells>,
}

impl Default for ViewportFrame {
    /// Of no world: what a frame taken off the wire is filled in.
    fn default() -> Self {
        Self { world: 0, seed: 0, worldgen_parameters: Vec::new(), drawn_heights: Default::default(), side_superchunks: None, tick: 0, ticks_a_second: 0.0, target_pace: 0, held_pace: None, entity_count: 0, counts: Vec::new(), sync_seconds: 0.0, sync_share: 0.0, viewport: None, hot: Vec::new(), pixel_side_log2: 0, seen_near: None, world_name: None, last_outcome: None, more_to_come: false, cells: Vec::new() }
    }
}

impl WiredStruct for FrameAsk {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|ask| &mut ask.viewport, |ask| &mut ask.pixel_side_log2, |ask| &mut ask.superchunks_skipped, |ask| &mut ask.most_superchunks, |ask| &mut ask.seen_near, |ask| &mut ask.planes, |ask| &mut ask.counted];
}

impl WiredStruct for NearRectangle {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|near| &mut near.first_cell_cartesian, |near| &mut near.size_in_cells, |near| &mut near.pixels_a_cell];
}

impl WiredStruct for Cells {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|cells| &mut cells.superchunk_cartesian, |cells| &mut cells.planes, |cells| &mut cells.top_left, |cells| &mut cells.entity_cells_cartesian];
}

impl WiredStruct for ViewportFrame {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|frame| &mut frame.world, |frame| &mut frame.seed, |frame| &mut frame.worldgen_parameters, |frame| &mut frame.drawn_heights, |frame| &mut frame.side_superchunks, |frame| &mut frame.tick, |frame| &mut frame.ticks_a_second, |frame| &mut frame.target_pace, |frame| &mut frame.held_pace, |frame| &mut frame.entity_count, |frame| &mut frame.counts, |frame| &mut frame.sync_seconds, |frame| &mut frame.sync_share, |frame| &mut frame.viewport, |frame| &mut frame.hot, |frame| &mut frame.pixel_side_log2, |frame| &mut frame.seen_near, |frame| &mut frame.world_name, |frame| &mut frame.last_outcome, |frame| &mut frame.more_to_come, |frame| &mut frame.cells];
}
