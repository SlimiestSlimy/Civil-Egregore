//! What a client asks of the world's viewport, and what it is answered
//! with: the hot superchunks it asks for ([`Ask`]), copied as the last
//! tick left them ([`Frame`]) -- their bitplanes' words as they are,
//! and where their sheep stand. Copying is all the host does for a
//! client: turning cells into pixels is the client's.

use crate::World;
use bitplane_manager::BucketKey;
use worldgen::GRASS;
use chunk_storage::{LayerType, SuperchunkImage};
use coordinates::{CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use worldgen::{TREE, TREE_STAGE};
pub use simulation::halos::Viewport;
use std::collections::HashMap;
use worldgen::{Generation, WET};

/// Words a chunk's bitmap takes.
pub const CHUNK_WORDS: usize = bitmap::WORDS;

/// Bits a word of a bitmap holds.
pub const WORD_BITS: usize = bitmap::BITS_PER_WORD;

/// Bits a cell's stage takes in [`Cells::stages`].
pub const STAGE_BITS: usize = TREE_STAGE.layer_type().bits() as usize;

/// The oldest stage a tree has.
pub const OLDEST_TREE_STAGE: u32 = worldgen::OLDEST_TREE_STAGE;

/// Where in its chunk, `(x, y)`, the cell is that the `bit`-th bit of
/// a chunk's bitmap stands for: the bits are in Morton order.
pub const fn cell_of_bit(bit: usize) -> (u8, u8) {
    bitmap::morton::morton_coordinates(bit)
}

/// The depth from which water hides what is under it: a power of two.
pub const DEEP: u32 = 16;

/// What a client wants of the world, one frame: some of the hot
/// superchunks of its viewport -- as many as a frame may carry, the
/// client going round them frame after frame -- and how finely it will
/// draw them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    /// What the client renders, none if it renders none of the world's
    /// cells (its map, say, drawn from generation alone).
    pub viewport: Option<Viewport>,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    /// Passed on in the frame; the host copies the same.
    pub detail: u32,
    /// The viewport's hot superchunks, row by row, to pass over first.
    pub skip: u32,
    /// Superchunks to answer with, at most.
    pub most: u32,
    /// The viewport's cells, if they are seen from near: drawn as one
    /// picture, a cell many pixels. Passed on in the frame.
    pub near: Option<Near>,
}

/// Cells seen from near: a rectangle of them, each many pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Near {
    /// The top left cell, `(x, y)` in the world.
    pub first: (u32, u32),
    /// Cells across and down.
    pub size: (u32, u32),
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
    let mut hot: Vec<(u32, u32)> = world.entities.superchunks().iter().map(|kept| kept.index().cartesian()).filter(|&at| viewport.contains(at)).collect();
    hot.sort_unstable_by_key(|&(x, y)| (y, x));
    hot
}

/// One hot superchunk's cells, as a tick left them.
pub struct Cells {
    /// Where it is in the world, `(x, y)` in superchunks.
    pub at: (u32, u32),
    /// Its grass: its 16 chunks' bitmaps one after another, in the
    /// chunks' Morton order, [`CHUNK_WORDS`] words each, in Morton order
    /// -- as the arena holds them. A chunk not hot is all clear.
    pub grass: Vec<u64>,
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
    /// chunk -- as the arena holds the plane.
    pub stages: Vec<u64>,
    /// The cells with water on them, however deep, laid out as the grass.
    pub wet: Vec<u64>,
    /// The low four bits of the water's depth, a bitplane a bit, each
    /// laid out as the grass.
    pub depths: [Vec<u64>; 4],
    /// The cells with water [`DEEP`] deep or more.
    pub deep: Vec<u64>,
    /// The cells its sheep stand on, `(x, y)` from its top left.
    pub sheep: Vec<(u16, u16)>,
}

/// The world's viewport, as a tick left it.
pub struct Frame {
    /// Counts the worlds the host has run: what a client made of an
    /// earlier one is made again.
    pub world: u64,
    /// The seed of the world run.
    pub seed: u64,
    /// How it is generated.
    pub generation: Generation,
    /// Superchunks along its side, a square about its origin, if it has
    /// a size.
    pub side: Option<u32>,
    /// Ticks run so far.
    pub tick: u64,
    /// Ticks a second, over the time since the frame before.
    pub ticks_a_second: f64,
    /// Sheep in the whole world.
    pub sheep: usize,
    /// Cells of grass in the whole world.
    pub grass: u64,
    /// Trees in the whole world.
    pub trees: u64,
    /// What answering took of the host's thread -- the counts and the
    /// copy, all a client costs it -- in seconds.
    pub sync_seconds: f64,
    /// The share of the thread's time that is, at the rate asked.
    pub sync_share: f64,
    /// What the client renders ([`Ask::viewport`]).
    pub viewport: Option<Viewport>,
    /// Every hot superchunk of the viewport, row by row: the client
    /// goes round them, and whatever else of the viewport it drew is
    /// cold now.
    pub hot: Vec<(u32, u32)>,
    /// How coarsely the client will draw them ([`Ask::detail`]).
    pub detail: u32,
    /// The cells seen from near ([`Ask::near`]).
    pub near: Option<Near>,
    /// The name of the world run, if it is one of the worlds' folder:
    /// opened from it, or saved to it.
    pub named: Option<String>,
    /// What opening or saving a world last came to, if either was asked.
    pub said: Option<String>,
    /// The superchunks asked for.
    pub cells: Vec<Cells>,
}

/// Cells of `layer_type` over every hot superchunk of `world`.
pub(crate) fn count(world: &World, layer_type: LayerType) -> u64 {
    world.arena.superchunk_indices().into_iter().map(|superchunk| world.arena.superchunk_count(layer_type, superchunk) as u64).sum()
}

/// The superchunks of `hot` -- hot ones of `world` -- that `ask` asks
/// for, copied: each one's planes, words as they are, and its sheep's
/// cells -- its heights and water only the first time it is copied,
/// `sent` keeping which.
pub(crate) fn copy(world: &World, hot: &[(u32, u32)], ask: Ask, sent: &mut HashMap<SuperchunkIndex, Water>) -> Vec<Cells> {
    let mut copied = Vec::new();
    for &(x, y) in hot.iter().skip(ask.skip as usize).take(ask.most as usize) {
        let superchunk = SuperchunkIndex::from_cartesian(x, y);
        let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
        let planes = |layer_type: LayerType| layer(world, layer_type, superchunk);
        // Heights never change: sent the once, in the first frame the superchunk is hot in. Nor does the water's depth, read off the image then and kept.
        let image = world.storage.image(superchunk).filter(|_| !sent.contains_key(&superchunk));
        let heights = image.map_or(Vec::new(), |image| image.height_words().to_vec());
        let Water { depths, deep } = sent.entry(superchunk).or_insert_with(|| Water::of(image)).clone();
        copied.push(Cells {
            at: (x, y),
            heights,
            top_left: (left, top),
            grass: planes(GRASS),
            trees: planes(TREE),
            stages: planes(TREE_STAGE.layer_type()),
            wet: planes(WET),
            depths,
            deep,
            sheep: sheep(world, superchunk),
        });
    }
    copied
}

/// A superchunk's water as a client draws it, off its image: kept for
/// every superchunk whose heights were sent.
#[derive(Clone, Default)]
pub(crate) struct Water {
    /// The low four bits of its depth, a bitplane a bit, laid out as
    /// the grass.
    depths: [Vec<u64>; 4],
    /// The cells [`DEEP`] deep or more.
    deep: Vec<u64>,
}

impl Water {
    /// The water of `image`: none of none, and of one with no water.
    fn of(image: Option<&SuperchunkImage>) -> Self {
        let words = CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS;
        let mut water = Self { depths: std::array::from_fn(|_| vec![0; words]), deep: vec![0; words] };
        let Some(image) = image else {
            return water;
        };
        for place in 0..words * u64::BITS as usize {
            let (depth, word, bit) = (image.depth(place) as u32, place / u64::BITS as usize, place % u64::BITS as usize);
            if depth >= DEEP {
                water.deep[word] |= 1 << bit;
                continue;
            }
            for (plane, depths) in water.depths.iter_mut().enumerate() {
                depths[word] |= ((depth >> plane & 1) as u64) << bit;
            }
        }
        water
    }
}

/// `superchunk`'s cells of `layer_type`: its chunks' words, one chunk
/// after another -- of a wide type, as many times a bitmap's words a
/// chunk as it has bits a cell.
fn layer(world: &World, layer_type: LayerType, superchunk: SuperchunkIndex) -> Vec<u64> {
    let chunk_words = CHUNK_WORDS * layer_type.bits() as usize;
    let mut words = Vec::with_capacity(CHUNKS_IN_SUPERCHUNK * chunk_words);
    for chunk in superchunk.chunks() {
        match world.arena.bucket(BucketKey { layer_type, chunk }) {
            Some(bucket) => words.extend_from_slice(bucket.words()),
            None => words.resize(words.len() + chunk_words, 0),
        }
    }
    words
}

/// The cells `superchunk`'s sheep stand on, from its top left.
fn sheep(world: &World, superchunk: SuperchunkIndex) -> Vec<(u16, u16)> {
    let Some(kept) = world.entities.superchunk(superchunk) else {
        return Vec::new();
    };
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    kept.iter().map(|entity| entity.header.at.cartesian()).map(|at| ((at.x - left) as u16, (at.y - top) as u16)).collect()
}
