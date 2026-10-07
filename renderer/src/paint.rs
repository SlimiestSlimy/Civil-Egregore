//! Cells into pixels, on a thread of its own: between the simulation,
//! which only copies the cells in view, and the window, which only
//! shows pixels. So drawing takes no time from the ticks, however much
//! of the world is in view, and none from the window's frames.
//!
//! A cell is a pixel: dirt brown, grass green, a sheep white, the
//! ground in the light its height gives it ([`crate::ground`]). From
//! near, a cell is several pixels and the cells in view are one
//! picture ([`crate::near`]). From far off, where
//! a pixel is many cells, it is their colours mixed: a tile of cells
//! `2^detail` a side is, in Morton order, a run of bits, so the grass
//! in it is counted from the words without a cell looked at.

use crate::ground::{lit, Given, Ground, COARSEST};
use crate::near::{paint_near, PaintedNear};
use crate::lab;
use crate::sim::{Cells, Frame, CHUNK_WORDS, DEEP};
use bitmap::morton::morton_coordinates;
use bitmap::BITS_PER_WORD;
use coordinates::{cartesian_from_place, CELLS_IN_CHUNK, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Instant;
use mc_rules::trees::OLDEST;

/// Dirt's colour.
pub const BROWN: [u8; 3] = [116, 80, 46];
/// Grass's colour.
pub const GREEN: [u8; 3] = [72, 160, 56];
/// A sheep's colour.
pub const WHITE: [u8; 3] = [240, 240, 236];

/// Pixels along a superchunk's side: a cell each.
const SIDE: usize = SUPERCHUNK_SIDE_CELLS as usize;

/// Cells from a sheep's own its square is drawn out to, each way: none,
/// a sheep a pixel, as it is a cell.
const SHEEP_REACH: usize = 0;

/// One superchunk's pixels.
pub struct Painted {
    /// Where it is in the world, `(x, y)` in superchunks.
    pub at: (u32, u32),
    /// Whether it is cold: nothing to draw, its pixels none.
    pub cold: bool,
    /// Pixels along its side: its cells', halved `detail` times.
    pub side: u32,
    /// Its pixels, row by row: red, green, blue, opacity.
    pub pixels: Vec<u8>,
}

/// A frame, painted.
pub struct Picture {
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
    /// What answering took of the simulation's thread, in seconds.
    pub sync_seconds: f64,
    /// The share of the thread's time that is.
    pub sync_share: f64,
    /// What painting took of the painter's thread, in seconds.
    pub paint_seconds: f64,
    /// The superchunks asked for, painted: seen from near, the cold
    /// ones only.
    pub superchunks: Vec<Painted>,
    /// The cells in view, if they are seen from near.
    pub near: Option<PaintedNear>,
}

/// Superchunks whose fine ground is kept, at most: 8 MiB each.
const FINE_KEPT: usize = 48;
/// Superchunks whose ground is kept at all, at most: past that, those
/// longest unseen go.
const GROUNDS_KEPT: usize = 2048;
/// Frames a ground past [`GROUNDS_KEPT`] may go unseen before it goes.
const UNSEEN_FRAMES: u64 = 256;

/// Makes the ground of every hot superchunk of `frame` that has none
/// yet, or none fine enough -- each on a thread of its own -- and
/// drops the fine parts of those longest unseen beyond [`FINE_KEPT`].
fn ground(grounds: &mut HashMap<(u32, u32), Ground>, frame: &Frame, number: u64) {
    let fine = frame.near.is_some() || frame.detail < 2;
    let hot = || frame.cells.iter().filter(|cells| cells.hot).map(|cells| cells.top_left);
    let missing: Vec<(u32, u32)> = hot().filter(|top_left| grounds.get(top_left).is_none_or(|ground| fine && ground.fine.is_none())).collect();
    let (seed, shape) = (lab::seed(), lab::generation().shape);
    // The heights the frame brings: those of the superchunks just turned hot.
    let given: Given = frame.cells.iter().filter(|cells| !cells.heights.is_empty()).map(|cells| (cells.top_left, &cells.heights[..])).collect();
    let given = &given;
    let made: Vec<Ground> = thread::scope(|scope| {
        let making: Vec<_> = missing.iter().map(|&top_left| scope.spawn(move || Ground::generate(seed, &shape, top_left, given))).collect();
        making.into_iter().map(|making| making.join().expect("a superchunk's ground")).collect()
    });
    grounds.extend(missing.into_iter().zip(made));
    for top_left in hot() {
        grounds.get_mut(&top_left).expect("made above").used = number;
    }
    let mut unseen: Vec<(u64, (u32, u32))> = grounds.iter().filter(|(_, ground)| ground.fine.is_some()).map(|(&top_left, ground)| (ground.used, top_left)).collect();
    unseen.sort_unstable();
    for &(used, top_left) in unseen.iter().take(unseen.len().saturating_sub(FINE_KEPT)) {
        if used < number {
            grounds.get_mut(&top_left).expect("listed above").coarsen();
        }
    }
    if grounds.len() > GROUNDS_KEPT {
        grounds.retain(|_, ground| ground.used + UNSEEN_FRAMES >= number);
    }
}

/// Starts the painter's thread: every frame from `frames` painted, and
/// sent on. It stops once either end is dropped.
pub fn start(frames: Receiver<Frame>) -> Receiver<Picture> {
    let (painted, pictures) = channel();
    thread::Builder::new()
        .name("painter".to_string())
        .spawn(move || {
            let (mut grounds, mut revision) = (HashMap::new(), 0);
            for (number, frame) in frames.into_iter().enumerate() {
                let started = Instant::now();
                if frame.revision != revision {
                    // The world is generated otherwise now: its ground is made again.
                    grounds.clear();
                    revision = frame.revision;
                }
                ground(&mut grounds, &frame, number as u64);
                let superchunks = frame
                    .cells
                    .iter()
                    .filter_map(|cells| match (cells.hot, frame.near, frame.detail) {
                        (false, _, _) => Some(Painted { at: cells.at, cold: true, side: 0, pixels: Vec::new() }),
                        // From near the cells in view are one picture.
                        (true, Some(_), _) => None,
                        (true, None, 0) => Some(paint(cells, &grounds[&cells.top_left])),
                        (true, None, detail) => Some(paint_far(cells, detail, &grounds[&cells.top_left])),
                    })
                    .collect();
                let near = frame.near.map(|near| paint_near(&frame.cells, &grounds, near));
                let picture = Picture {
                    tick: frame.tick,
                    ticks_a_second: frame.ticks_a_second,
                    sheep: frame.sheep,
                    grass: frame.grass,
                    trees: frame.trees,
                    sync_seconds: frame.sync_seconds,
                    sync_share: frame.sync_share,
                    paint_seconds: started.elapsed().as_secs_f64(),
                    superchunks,
                    near,
                };
                if painted.send(picture).is_err() {
                    return;
                }
            }
        })
        .expect("a thread for the painter");
    pictures
}

/// How far across and down from its superchunk's top left the chunk at
/// `place` starts, in cells.
fn chunk_top_left(place: usize) -> (usize, usize) {
    let (x, y) = cartesian_from_place(place * CELLS_IN_CHUNK);
    (x as usize, y as usize)
}

/// Water.
pub const WATER: [u8; 3] = [30, 92, 168];
/// How much of what is under it a film of water hides, of [`DEEP`]:
/// water one deep hides this and one more, and so on to all of it.
const FILM: usize = 4;

/// How deep the water at bit `bit` of word `word` of `cells`' bitmaps
/// is, to [`DEEP`] at most.
pub fn depth_at(cells: &Cells, word: usize, bit: u32) -> u32 {
    if cells.deep[word] >> bit & 1 == 1 {
        return DEEP;
    }
    (0..cells.depths.len()).map(|plane| ((cells.depths[plane][word] >> bit & 1) as u32) << plane).sum()
}

/// `colour` under water `depth` deep: the less of it seen the deeper,
/// none from [`DEEP`].
pub fn under_water(colour: [u8; 3], depth: u32) -> [u8; 3] {
    mixed(colour, WATER, FILM + depth as usize, FILM + DEEP as usize)
}

/// A tree at its first stage, and at its last: darker as it ages.
const TREE_YOUNG: [u8; 3] = [62, 128, 44];
/// A tree at its last stage.
const TREE_OLD: [u8; 3] = [14, 62, 30];

/// Bits a cell of the trees' stage plane.
const STAGE_BITS: usize = mc_rules::trees::TREE_STAGE.layer_type().bits() as usize;

/// A tree's colour at `stage`.
pub fn tree_colour(stage: u32) -> [u8; 3] {
    mixed(TREE_YOUNG, TREE_OLD, stage as usize, OLDEST as usize)
}

/// The stage of the tree at bit `bit` of word `word` of `cells`' bitmaps.
pub fn stage_at(cells: &Cells, word: usize, bit: u32) -> u32 {
    // The cell's place among the superchunk's, and its stage's four bits there.
    let at = (word * BITS_PER_WORD + bit as usize) * STAGE_BITS;
    (cells.stages[at / BITS_PER_WORD] >> (at % BITS_PER_WORD)) as u32 & ((1 << STAGE_BITS) - 1)
}

/// `colour`, opaque.
const fn opaque(colour: [u8; 3]) -> [u8; 4] {
    [colour[0], colour[1], colour[2], u8::MAX]
}

/// A superchunk's cells as pixels: dirt, its grass over it, both in
/// the `ground`'s light, and its sheep over that.
fn paint(cells: &Cells, ground: &Ground) -> Painted {
    let mut pixels = vec![opaque(BROWN); SIDE * SIDE];
    let green = opaque(GREEN);
    for (place, chunk) in cells.grass.as_chunks::<CHUNK_WORDS>().0.iter().enumerate() {
        let (left, top) = chunk_top_left(place);
        for (word_index, &word) in chunk.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let (x, y) = morton_coordinates(word_index * BITS_PER_WORD + bits.trailing_zeros() as usize);
                pixels[(top + y as usize) * SIDE + left + x as usize] = green;
                bits &= bits - 1;
            }
        }
    }
    // Its trees over the grass, each in its stage's colour.
    for (index, &word) in cells.trees.iter().enumerate() {
        let (left, top) = chunk_top_left(index / CHUNK_WORDS);
        let mut bits = word;
        while bits != 0 {
            let bit = bits.trailing_zeros();
            let (x, y) = morton_coordinates(index % CHUNK_WORDS * BITS_PER_WORD + bit as usize);
            pixels[(top + y as usize) * SIDE + left + x as usize] = opaque(tree_colour(stage_at(cells, index, bit)));
            bits &= bits - 1;
        }
    }
    for (pixel, &factor) in pixels.iter_mut().zip(&ground.levels[0]) {
        *pixel = opaque(lit([pixel[0], pixel[1], pixel[2]], factor));
    }
    // Its water over the lit ground: the deeper, the less of the ground seen.
    for (index, &word) in cells.wet.iter().enumerate() {
        let (left, top) = chunk_top_left(index / CHUNK_WORDS);
        let mut bits = word;
        while bits != 0 {
            let bit = bits.trailing_zeros();
            let (x, y) = morton_coordinates(index % CHUNK_WORDS * BITS_PER_WORD + bit as usize);
            let pixel = &mut pixels[(top + y as usize) * SIDE + left + x as usize];
            *pixel = opaque(under_water([pixel[0], pixel[1], pixel[2]], depth_at(cells, index, bit)));
            bits &= bits - 1;
        }
    }
    let white = opaque(WHITE);
    for &(x, y) in &cells.sheep {
        let (x, y) = (x as usize, y as usize);
        for y in y.saturating_sub(SHEEP_REACH)..=(y + SHEEP_REACH).min(SIDE - 1) {
            for x in x.saturating_sub(SHEEP_REACH)..=(x + SHEEP_REACH).min(SIDE - 1) {
                pixels[y * SIDE + x] = white;
            }
        }
    }
    Painted { at: cells.at, cold: false, side: SIDE as u32, pixels: pixels.into_flattened() }
}

/// `from` and `to` mixed, `part` of `whole` of it `to`.
fn mixed(from: [u8; 3], to: [u8; 3], part: usize, whole: usize) -> [u8; 3] {
    let part = part.min(whole);
    std::array::from_fn(|channel| ((from[channel] as usize * (whole - part) + to[channel] as usize * part) / whole) as u8)
}

/// The cells set in each tile of cells `2^detail` a side of a
/// superchunk's `words` -- its chunks' bitmaps one after another --
/// row by row: each tile a run of bits in Morton order, counted from
/// the words with no cell looked at.
fn counted(words: &[u64], detail: u32) -> Vec<u16> {
    let (side, tile_cells) = (SIDE >> detail, 1usize << (2 * detail));
    let mut counts = vec![0u16; side * side];
    for (place, chunk) in words.as_chunks::<CHUNK_WORDS>().0.iter().enumerate() {
        let (left, top) = chunk_top_left(place);
        let (left, top) = (left >> detail, top >> detail);
        for tile in 0..CELLS_IN_CHUNK / tile_cells {
            let count: u32 = if tile_cells >= BITS_PER_WORD {
                let words = tile_cells / BITS_PER_WORD;
                chunk[tile * words..][..words].iter().map(|word| word.count_ones()).sum()
            } else {
                let bit = tile * tile_cells;
                (chunk[bit / BITS_PER_WORD] >> (bit % BITS_PER_WORD) & ((1 << tile_cells) - 1)).count_ones()
            };
            let (x, y) = morton_coordinates(tile);
            counts[(top + y as usize) * side + left + x as usize] = count as u16;
        }
    }
    counts
}

/// A superchunk's cells as pixels from far off, a pixel a tile of
/// cells `2^detail` a side: dirt and grass mixed by the grass in the
/// tile, its trees' colour mixed in by the trees in it, and white mixed in by the
/// sheep on it, each as many cells as it is drawn from near; the
/// ground in the `ground`'s light.
fn paint_far(cells: &Cells, detail: u32, ground: &Ground) -> Painted {
    let (side, tile_cells) = (SIDE >> detail, 1usize << (2 * detail));
    let (grass, trees) = (counted(&cells.grass, detail), counted(&cells.trees, detail));
    let (wet, deep) = (counted(&cells.wet, detail), counted(&cells.deep, detail));
    let mut sheep = vec![0u16; side * side];
    for &(x, y) in &cells.sheep {
        let at = (y as usize >> detail) * side + (x as usize >> detail);
        sheep[at] = sheep[at].saturating_add(1);
    }
    let sheep_cells = (2 * SHEEP_REACH + 1) * (2 * SHEEP_REACH + 1);
    let mut pixels = Vec::with_capacity(side * side * 4);
    for (index, &factor) in ground.levels[(detail as usize).min(COARSEST)].iter().enumerate() {
        let (grass, trees, sheep) = (grass[index], trees[index], sheep[index]);
        let ground = mixed(mixed(BROWN, GREEN, grass as usize, tile_cells), tree_colour(OLDEST / 2), trees as usize, tile_cells);
        // The water over the lit ground, by the share of the tile under it: shallow water half seen through.
        let ground = mixed(lit(ground, factor), WATER, (wet[index] as usize + deep[index] as usize) / 2, tile_cells);
        pixels.extend_from_slice(&opaque(mixed(ground, WHITE, sheep as usize * sheep_cells, tile_cells)));
    }
    Painted { at: cells.at, cold: false, side: side as u32, pixels }
}
