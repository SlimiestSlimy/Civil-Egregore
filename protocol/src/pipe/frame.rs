//! What a client asks of the world's viewport ([`FrameAsk`]) and what it is
//! answered with ([`ViewportFrame`]): the hot superchunks asked for, each
//! copied as a tick left it and nothing more -- and how a client reads
//! the planes a frame brings.

pub use base_types::Climate;
use base_types::{CLIMATE, CLOUD, TREE_STAGE};
pub use simulation::halos::Viewport;
use utilities::wire::{Wired, WiredStruct};
use worldgen::WorldGeneration;

/// Words a chunk's bitmap takes.
pub const CHUNK_WORDS: usize = bitmap::WORDS;

/// Bits a word of a bitmap holds.
pub const WORD_BITS: usize = bitmap::BITS_PER_WORD;

/// Bits a cell's stage takes in [`Cells::stages`].
pub const STAGE_BITS: usize = TREE_STAGE.layer_type().width_bits() as usize;

/// The oldest stage a tree has.
pub const OLDEST_TREE_STAGE: u32 = base_types::OLDEST_TREE_STAGE;

/// Bits a tile's cloud takes in [`Cells::clouds`]: the cloud plane's,
/// as the type registry has it.
pub const CLOUD_BITS: usize = CLOUD.layer_type().width_bits() as usize;
/// How many places a cell's place in its superchunk is shifted by to
/// give its tile's in the cloud plane: the plane's tile size, as the
/// type registry has it.
pub const CLOUD_TILE_SHIFT: u32 = CLOUD.layer_type().tile_shift();
/// The most cloud a tile holds: the deepest shadow.
pub const DEEPEST_CLOUD: u32 = CLOUD.most_value();

/// How many places a cell's place in its chunk is shifted by to give
/// its climate's tile's, as the type registry has it.
pub const CLIMATE_TILE_SHIFT: u32 = CLIMATE.layer_type().tile_shift();
/// Bits a tile's climate takes in [`Cells::climates`]: the climate
/// plane's.
pub const CLIMATE_BITS: usize = CLIMATE.layer_type().width_bits() as usize;

/// The climate of the tile at `tile` of a superchunk -- its tiles in
/// Morton order, as [`Cells::climates`] holds them: none where no
/// climate was sent.
pub fn climate_over(climates: &[u64], tile: usize) -> Option<Climate> {
    let first_bit = tile * CLIMATE_BITS;
    climates.get(first_bit / WORD_BITS).map(|word| Climate::of_number((word >> (first_bit % WORD_BITS)) as u32 & CLIMATE.most_value()))
}

/// The cloud over the tile at `tile` of a superchunk -- its tiles in
/// Morton order, as [`Cells::clouds`] holds them: 0 for none, and
/// where no cloud was sent.
pub fn cloud_over(clouds: &[u64], tile: usize) -> u32 {
    let first_bit = tile * CLOUD_BITS;
    clouds.get(first_bit / WORD_BITS).map_or(0, |word| (word >> (first_bit % WORD_BITS)) as u32 & DEEPEST_CLOUD)
}

/// Where in its chunk, `(x, y)`, the cell is that the `bit_index`-th bit of
/// a chunk's bitmap stands for: the bits are in Morton order.
pub const fn cell_of_bit(bit_index: usize) -> (u8, u8) {
    bitmap::morton::morton_coordinates(bit_index)
}

/// The height of the cell at `place` in its superchunk, from the height
/// words a frame brings ([`Cells::heights`]).
pub fn height_in_frame(height_words: &[u64], place: usize) -> super::terrain::Height {
    chunk_storage::height_in(height_words, place)
}

/// The depth from which water hides what is under it: a power of two.
pub const DEEP_WATER_DEPTH: u32 = 16;

/// What a client wants of the world, one frame: some of the hot
/// superchunks of its viewport -- as many as a frame may carry, the
/// client going round them frame after frame -- and how finely it will
/// draw them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// Its grass: its 16 chunks' bitmaps one after another, in the
    /// chunks' Morton order, [`CHUNK_WORDS`] words each, in Morton order
    /// -- as the arena holds them. A chunk not hot is all clear.
    pub grass_bitmaps: Vec<u64>,
    /// Its top left cell in the world, `(x, y)`: what its ground is
    /// worked out from.
    pub top_left: (u32, u32),
    /// Its heights, as its image holds them (`chunk_storage::height_in`),
    /// in the first frame it is hot in and empty in every other: the
    /// client is spared working them out again.
    pub heights: Vec<u64>,
    /// Its trees, laid out as the grass.
    pub trees: Vec<u64>,
    /// Its trees' stages, four bits a cell: its 16 chunks' buckets one
    /// after another, a cell's stage at four times its place in its
    /// chunk -- as the arena holds the plane. Empty where no stage is
    /// drawn: asked for coarser than a cell a pixel, and not from near.
    pub stages: Vec<u64>,
    /// The cells with water on them, however deep, laid out as the grass.
    pub wet_bitmaps: Vec<u64>,
    /// Its clouds, [`CLOUD_BITS`] a tile: its 16 chunks' buckets one
    /// after another, as the arena holds the plane -- a number a tile
    /// of the plane's tile size, read by [`cloud_over`]. Empty where
    /// no tile of it has any cloud.
    pub clouds: Vec<u64>,
    /// Which of its tiles are raining, a bit a tile in the order of
    /// `clouds`, read by [`rains_over`]. Empty where none is.
    pub raining_tiles: Vec<u64>,
    /// Its climates, [`CLIMATE_BITS`] a tile, laid out as the
    /// clouds, read by [`climate_over`]: what a client writes on a
    /// tile when asked to show its climate.
    pub climates: Vec<u64>,
    /// How deep its water is, in the frame its heights come in and in
    /// no other: it never changes, and the client keeps it.
    pub water: Option<Water>,
    /// The cells its sheep stand on, `(x, y)` from its top left.
    pub sheep_cells_cartesian: Vec<(u16, u16)>,
}

/// The world's viewport, as a tick left it.
#[derive(Clone)]
pub struct ViewportFrame {
    /// Counts the worlds the host has run: what a client made of an
    /// earlier one is made again.
    pub world: u64,
    /// The seed of the world run.
    pub seed: u64,
    /// How it is generated.
    pub world_generation: WorldGeneration,
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
    /// Sheep in the whole world.
    pub sheep_count: usize,
    /// Cells of grass in the whole world.
    pub grass_cells: u64,
    /// Trees in the whole world.
    pub trees: u64,
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

/// The tiles of a superchunk that are raining, a bit a tile in the
/// order of `clouds`, its tiles' clouds: those whose cloud is above
/// what their climate -- `climates`, the climate plane's words for
/// the same tiles -- lets the air hold (`Climate::rains_above`).
/// Empty if none is.
pub fn raining_tiles_of(clouds: &[u64], climates: &[u64]) -> Vec<u64> {
    let mut raining_bits = vec![0u64; (clouds.len() * WORD_BITS / CLOUD_BITS).div_ceil(WORD_BITS)];
    for tile in (0..clouds.len() * WORD_BITS / CLOUD_BITS).filter(|&tile| cloud_over(clouds, tile) != 0) {
        if climate_over(climates, tile).unwrap_or(Climate::of_number(0)).rains(cloud_over(clouds, tile)) {
            raining_bits[tile / WORD_BITS] |= 1 << (tile % WORD_BITS);
        }
    }
    if raining_bits.iter().any(|&word| word != 0) { raining_bits } else { Vec::new() }
}

/// Whether the tile at `tile` of a superchunk is raining, by
/// [`Cells::raining_tiles`]: not where none was sent.
pub fn rains_over(raining_tiles: &[u64], tile: usize) -> bool {
    raining_tiles.get(tile / WORD_BITS).is_some_and(|word| word >> (tile % WORD_BITS) & 1 == 1)
}

/// A superchunk's water as a client draws it, off its image: kept for
/// every superchunk whose heights were sent.
#[derive(Clone, Default)]
pub struct Water {
    /// The low four bits of its depth, a bitplane a bit, laid out as
    /// the grass.
    pub depth_low_bits: [Vec<u64>; 4],
    /// The cells [`DEEP_WATER_DEPTH`] deep or more.
    pub deep_cells: Vec<u64>,
}

impl Default for ViewportFrame {
    /// Of no world: what a frame taken off the wire is filled in.
    fn default() -> Self {
        Self { world: 0, seed: 0, world_generation: WorldGeneration::DEFAULT, drawn_heights: Default::default(), side_superchunks: None, tick: 0, ticks_a_second: 0.0, sheep_count: 0, grass_cells: 0, trees: 0, sync_seconds: 0.0, sync_share: 0.0, viewport: None, hot: Vec::new(), pixel_side_log2: 0, seen_near: None, world_name: None, last_outcome: None, more_to_come: false, cells: Vec::new() }
    }
}

impl WiredStruct for FrameAsk {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|ask| &mut ask.viewport, |ask| &mut ask.pixel_side_log2, |ask| &mut ask.superchunks_skipped, |ask| &mut ask.most_superchunks, |ask| &mut ask.seen_near];
}

impl WiredStruct for NearRectangle {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|near| &mut near.first_cell_cartesian, |near| &mut near.size_in_cells, |near| &mut near.pixels_a_cell];
}

impl WiredStruct for Water {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|water| &mut water.depth_low_bits, |water| &mut water.deep_cells];
}

impl WiredStruct for Cells {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|cells| &mut cells.superchunk_cartesian, |cells| &mut cells.grass_bitmaps, |cells| &mut cells.top_left, |cells| &mut cells.heights, |cells| &mut cells.trees, |cells| &mut cells.stages, |cells| &mut cells.wet_bitmaps, |cells| &mut cells.clouds, |cells| &mut cells.raining_tiles, |cells| &mut cells.climates, |cells| &mut cells.water, |cells| &mut cells.sheep_cells_cartesian];
}

impl WiredStruct for ViewportFrame {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|frame| &mut frame.world, |frame| &mut frame.seed, |frame| &mut frame.world_generation, |frame| &mut frame.drawn_heights, |frame| &mut frame.side_superchunks, |frame| &mut frame.tick, |frame| &mut frame.ticks_a_second, |frame| &mut frame.sheep_count, |frame| &mut frame.grass_cells, |frame| &mut frame.trees, |frame| &mut frame.sync_seconds, |frame| &mut frame.sync_share, |frame| &mut frame.viewport, |frame| &mut frame.hot, |frame| &mut frame.pixel_side_log2, |frame| &mut frame.seen_near, |frame| &mut frame.world_name, |frame| &mut frame.last_outcome, |frame| &mut frame.more_to_come, |frame| &mut frame.cells];
}
