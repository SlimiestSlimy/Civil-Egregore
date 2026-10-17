//! What a client asks of the world's viewport ([`FrameAsk`]) and what it is
//! answered with ([`ViewportFrame`]): the hot superchunks asked for, each
//! copied as a tick left it and nothing more
//! (`docs/server.md`, "The host").

use crate::World;
use bitplane_manager::BucketKey;
use base_types::GRASS;
use chunk_storage::{LayerType, SuperchunkImage};
use coordinates::{CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
pub use base_types::Climate;
use base_types::{CLIMATE, CLOUD, TREE, TREE_STAGE};
pub use simulation::halos::Viewport;
use std::collections::HashMap;
use std::sync::Arc;
use worldgen::WorldGeneration;
use base_types::WET;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NearRectangle {
    /// The top left cell, `(x, y)` in the world.
    pub first_cell_cartesian: (u32, u32),
    /// Cells across and down.
    pub size_in_cells: (u32, u32),
    /// Pixels along a cell's side: 2, 4 or 8.
    pub pixels_a_cell: u32,
}

/// The hot superchunks of `world` in `viewport`, each `(x, y)` in the
/// world, row by row: none if there is no viewport. Hot superchunks are
/// few, however wide the viewport.
pub(crate) fn hot_in(world: &World, viewport: Option<Viewport>) -> Vec<(u32, u32)> {
    let Some(viewport) = viewport else {
        return Vec::new();
    };
    let mut hot: Vec<(u32, u32)> = world.entity_store().superchunks().iter().map(|superchunk_entities| superchunk_entities.superchunk_index().cartesian()).filter(|&superchunk_cartesian| viewport.contains(superchunk_cartesian)).collect();
    hot.sort_unstable_by_key(|&(x, y)| (y, x));
    hot
}

/// One hot superchunk's cells, as a tick left them.
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
    /// How deep its water is: never changing, so shared with every
    /// frame it is in, not copied.
    pub water: Arc<Water>,
    /// The cells its sheep stand on, `(x, y)` from its top left.
    pub sheep_cells_cartesian: Vec<(u16, u16)>,
}

/// The world's viewport, as a tick left it.
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

/// A frame of `world`, the `worlds`-th a client is shown: what `frame_ask`
/// asked, `hot` the viewport's hot superchunks, `cells` those copied
/// for it; `measured` the ticks a second, the seconds the copy took
/// and the share of the time that is; the world's name and what was
/// last said of it; and whether `more_to_come` of what was asked is to come.
/// The one making of a frame: the host's thread's and a shown run's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_of(world: &World, worlds: u64, frame_ask: FrameAsk, hot: Vec<(u32, u32)>, cells: Vec<Cells>, (ticks_a_second, sync_seconds, sync_share): (f64, f64, f64), (world_name, last_outcome): (Option<String>, Option<String>), more_to_come: bool) -> ViewportFrame {
    let (sheep_count, grass_cells, trees) = (world.entity_store().len(), count(world, GRASS), count(world, TREE));
    ViewportFrame { world: worlds, seed: world.world_info.seed, world_generation: world.world_generation, drawn_heights: super::terrain::DrawnHeightRange::from_generation(&world.world_generation), side_superchunks: world.halos().hot.world_side_superchunks(), tick: world.entity_store().current_tick(), ticks_a_second, sheep_count, grass_cells, trees, sync_seconds, sync_share, viewport: frame_ask.viewport, hot, pixel_side_log2: frame_ask.pixel_side_log2, seen_near: frame_ask.seen_near, world_name, last_outcome, more_to_come, cells }
}

/// Cells of `layer_type` over every hot superchunk of `world`.
pub(crate) fn count(world: &World, layer_type: LayerType) -> u64 {
    world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(layer_type, superchunk) as u64).sum()
}

/// The superchunk at `(x, y)` of `world` copied as `frame_ask` asks, if it
/// is hot still: its planes, words as they are, and its sheep's cells
/// -- its heights only the first time it is copied, its water read off
/// its image then and shared from then on, `heights_sent` keeping which.
pub(crate) fn copy_superchunk(world: &World, (x, y): (u32, u32), frame_ask: FrameAsk, heights_sent: &mut HashMap<SuperchunkIndex, Arc<Water>>) -> Option<Cells> {
    let superchunk = SuperchunkIndex::from_cartesian(x, y);
    world.entity_store().superchunk(superchunk)?;
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    let plane_words = |layer_type: LayerType| layer(world, layer_type, superchunk);
    // Heights never change: sent the once, in the first frame the superchunk is hot in. Nor does the water's depth.
    let image = world.chunk_storage().image(superchunk).filter(|_| !heights_sent.contains_key(&superchunk));
    let heights = image.map_or(Vec::new(), |image| image.height_words().to_vec());
    let water = Arc::clone(heights_sent.entry(superchunk).or_insert_with(|| Arc::new(Water::from_image(image))));
    // A tree's stage is drawn only where a cell is a pixel or more.
    let stages = if frame_ask.seen_near.is_some() || frame_ask.pixel_side_log2 == 0 { plane_words(TREE_STAGE.layer_type()) } else { Vec::new() };
    // No cloud over it: nothing to send, nothing to draw.
    let clouds = Some(plane_words(CLOUD.layer_type())).filter(|clouds| clouds.iter().any(|&word| word != 0)).unwrap_or_default();
    // Which of its tiles rain is the climate's to say, of each tile's own: worked out here, so a client need not.
    let climates = plane_words(CLIMATE.layer_type());
    let raining_tiles = if clouds.is_empty() { Vec::new() } else { raining_tiles_of(&clouds, &climates) };
    Some(Cells { superchunk_cartesian: (x, y), heights, top_left: (left_cell_x, top_cell_y), grass_bitmaps: plane_words(GRASS), trees: plane_words(TREE), stages, wet_bitmaps: plane_words(WET), clouds, raining_tiles, climates, water, sheep_cells_cartesian: sheep_cells_of(world, superchunk) })
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

impl Water {
    /// The water of `image`: none of none, and of one with no water.
    fn from_image(image: Option<&SuperchunkImage>) -> Self {
        let words = CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS;
        let mut water = Self { depth_low_bits: std::array::from_fn(|_| vec![0; words]), deep_cells: vec![0; words] };
        let Some(image) = image else {
            return water;
        };
        for place in 0..words * u64::BITS as usize {
            let (cell_depth, word, bit_in_word) = (image.water_depth(place) as u32, place / u64::BITS as usize, place % u64::BITS as usize);
            if cell_depth >= DEEP_WATER_DEPTH {
                water.deep_cells[word] |= 1 << bit_in_word;
                continue;
            }
            for (depth_bit, depth_bitmap) in water.depth_low_bits.iter_mut().enumerate() {
                depth_bitmap[word] |= ((cell_depth >> depth_bit & 1) as u64) << bit_in_word;
            }
        }
        water
    }
}

/// `superchunk`'s cells of `layer_type`: its chunks' words, one chunk
/// after another -- of a wide type, as many times a bitmap's words a
/// chunk as it has bits a cell.
fn layer(world: &World, layer_type: LayerType, superchunk: SuperchunkIndex) -> Vec<u64> {
    let chunk_words = chunk_storage::wide::bucket_words(layer_type);
    let mut words = Vec::with_capacity(CHUNKS_IN_SUPERCHUNK * chunk_words);
    for chunk in superchunk.chunks() {
        match world.bitmap_arena().bucket(BucketKey { layer_type, chunk }) {
            Some(bucket) => words.extend_from_slice(bucket.words()),
            None => words.resize(words.len() + chunk_words, 0),
        }
    }
    words
}

/// The cells `superchunk`'s sheep stand on, from its top left.
fn sheep_cells_of(world: &World, superchunk: SuperchunkIndex) -> Vec<(u16, u16)> {
    let Some(superchunk_entities) = world.entity_store().superchunk(superchunk) else {
        return Vec::new();
    };
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    superchunk_entities.iter().map(|entity| entity.header.stood_cell.cartesian()).map(|sheep_cell| ((sheep_cell.x - left_cell_x) as u16, (sheep_cell.y - top_cell_y) as u16)).collect()
}
