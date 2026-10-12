//! Cells into pixels, on a thread of its own, between the host, which
//! only copies, and the window, which only shows
//! (`docs/renderer.md`, "Three threads" and "Many superchunks").

use crate::ground::{lit, Given, Ground, COARSEST};
use crate::near::{paint_near, PaintedNear};
use coordinates::{cartesian_from_place, CELLS_IN_CHUNK, SUPERCHUNK_SIDE_CELLS};
use server::host::frame::{cell_of_bit, cloud_over, rains_over, Cells, Frame, CHUNK_WORDS, CLOUD_TILE_SHIFT, DEEP, DEEPEST_CLOUD, OLDEST_TREE_STAGE, STAGE_BITS, WORD_BITS};
use server::host::terrain::TerrainAsker;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Instant;
use utilities::tuning::Tuning;

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
    /// Pixels along its side: its cells', halved `detail` times.
    pub side: u32,
    /// Its pixels, row by row: red, green, blue, opacity.
    pub pixels: Vec<u8>,
}

/// A frame, painted.
pub struct Picture {
    /// The frame, its cells gone into the pixels.
    pub frame: Frame,
    /// What painting took of the painter's thread, in seconds.
    pub paint_seconds: f64,
    /// The superchunks asked for, painted: none seen from near.
    pub superchunks: Vec<Painted>,
    /// The viewport's cells, if they are seen from near.
    pub near: Option<PaintedNear>,
}

/// Superchunks whose fine ground is kept, at most: 8 MiB each.
const FINE_KEPT: usize = 48;
/// Superchunks whose ground is kept at all, at most: past that, those
/// longest unseen go.
const GROUNDS_KEPT: usize = 2048;
/// Frames a ground past [`GROUNDS_KEPT`] may go unseen before it goes.
const UNSEEN_FRAMES: u64 = 256;

/// Makes the ground of every superchunk of `frame` -- each hot -- that has none
/// yet, or none fine enough -- each on a thread of its own -- and
/// drops the fine parts of those longest unseen beyond [`FINE_KEPT`].
fn ground(grounds: &mut HashMap<(u32, u32), Ground>, terrain: &TerrainAsker, frame: &Frame, number: u64) {
    let fine = frame.near.is_some() || frame.detail < 2;
    let hot = || frame.cells.iter().map(|cells| cells.top_left);
    let missing: Vec<(u32, u32)> = hot().filter(|top_left| grounds.get(top_left).is_none_or(|ground| fine && ground.fine.is_none())).collect();
    let (world, levels) = (frame.world, frame.levels);
    // The heights the frame brings: those of the superchunks just turned hot.
    let given: Given = frame.cells.iter().filter(|cells| !cells.heights.is_empty()).map(|cells| (cells.top_left, &cells.heights[..])).collect();
    let given = &given;
    let made: Vec<Ground> = thread::scope(|scope| {
        let making: Vec<_> = missing.iter().map(|&top_left| scope.spawn(move || Ground::ask(terrain, world, levels, top_left, given))).collect();
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
/// sent on, shaded from near as the last of `tunings` says -- as the
/// default settings do until one comes -- the heights about a
/// superchunk asked of `terrain`. It stops once either end is
/// dropped.
pub fn start(frames: Receiver<Frame>, tunings: Receiver<Tuning>, terrain: TerrainAsker) -> Receiver<Picture> {
    let (painted, pictures) = channel();
    thread::Builder::new()
        .name("painter".to_string())
        .spawn(move || {
            let (mut grounds, mut world, mut tuning) = (HashMap::new(), 0, utilities::tuning::defaults());
            for (number, mut frame) in frames.into_iter().enumerate() {
                let started = Instant::now();
                if let Some(latest) = tunings.try_iter().last() {
                    tuning = latest;
                }
                if frame.world != world {
                    // Another world: its ground is its own.
                    grounds.clear();
                    world = frame.world;
                }
                ground(&mut grounds, &terrain, &frame, number as u64);
                let superchunks = frame
                    .cells
                    .iter()
                    .filter_map(|cells| match (frame.near, frame.detail) {
                        // From near the viewport's cells are one picture.
                        (Some(_), _) => None,
                        (None, 0) => Some(paint(cells, &grounds[&cells.top_left])),
                        (None, detail) => Some(paint_far(cells, detail, &grounds[&cells.top_left])),
                    })
                    .collect();
                let near = frame.near.map(|near| paint_near(&frame.cells, &grounds, near, &tuning));
                frame.cells = Vec::new();
                let picture = Picture { frame, paint_seconds: started.elapsed().as_secs_f64(), superchunks, near };
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
/// How much of what is under it a film of water hides, beside the
/// [`DEEP`] that hide all of it: water one deep hides this and one
/// more, and so on -- three quarters from the first, so the shallowest
/// water is told from wet ground.
const FILM: usize = 44;

/// How deep the water at bit `bit` of word `word` of `cells`' bitmaps
/// is, to [`DEEP`] at most.
pub fn depth_at(cells: &Cells, word: usize, bit: u32) -> u32 {
    if cells.water.deep[word] >> bit & 1 == 1 {
        return DEEP;
    }
    (0..cells.water.depths.len()).map(|plane| ((cells.water.depths[plane][word] >> bit & 1) as u32) << plane).sum()
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

/// A tree's colour at `stage`.
pub fn tree_colour(stage: u32) -> [u8; 3] {
    mixed(TREE_YOUNG, TREE_OLD, stage as usize, OLDEST_TREE_STAGE as usize)
}

/// The stage of the tree at bit `bit` of word `word` of `cells`' bitmaps.
pub fn stage_at(cells: &Cells, word: usize, bit: u32) -> u32 {
    // The cell's place among the superchunk's, and its stage's four bits there.
    let at = (word * WORD_BITS + bit as usize) * STAGE_BITS;
    (cells.stages[at / WORD_BITS] >> (at % WORD_BITS)) as u32 & ((1 << STAGE_BITS) - 1)
}

/// What a cloud's shadow darkens towards: dark, and a little blue.
const CLOUD_SHADOW: [u8; 3] = [18, 22, 40];
/// How much of what is under it the deepest cloud's shadow leaves: a
/// half -- the ground is still read under it.
const SEEN_UNDER_DEEPEST_CLOUD: usize = 2;

/// `colour` in the shadow of `cloud` of `whole`: none at 0, the
/// deepest at `whole` -- [`DEEPEST_CLOUD`], or as many times it as
/// the clouds added up.
pub fn under_cloud(colour: [u8; 3], cloud: usize, whole: usize) -> [u8; 3] {
    mixed(colour, CLOUD_SHADOW, cloud, SEEN_UNDER_DEEPEST_CLOUD * whole)
}

/// What rain is drawn in: a pale blue.
const RAIN: [u8; 3] = [150, 176, 226];
/// Of this many cells under a raining cloud, one has a streak of rain
/// drawn on it.
pub const CELLS_A_STREAK_OF_RAIN: usize = 5;

/// Whether the cell `(x, y)` -- counted from anywhere, the same for
/// every tile -- has a streak of rain drawn on it under a raining
/// cloud: one in [`CELLS_A_STREAK_OF_RAIN`], in slanted lines.
pub fn streak_of_rain_on(x: usize, y: usize) -> bool {
    (2 * x + y).is_multiple_of(CELLS_A_STREAK_OF_RAIN)
}

/// `colour` with `rain` of `whole` of it rain: a streak, whole, half
/// seen through.
pub fn in_rain(colour: [u8; 3], rain: usize, whole: usize) -> [u8; 3] {
    mixed(colour, RAIN, rain, 2 * whole)
}

/// Every tile of `cells` with a cloud over it: where its top left
/// cell is from the superchunk's, `(x, y)`, how many cells it is a
/// side, its cloud, and whether it is raining, as the frame says
/// (`Cells::raining`) -- the tiles those of the cloud plane, its tile
/// size the type registry's.
pub fn each_cloud(cells: &Cells, mut shadow: impl FnMut((usize, usize), usize, u32, bool)) {
    let (tiles_a_chunk, side) = (CELLS_IN_CHUNK >> CLOUD_TILE_SHIFT, 1usize << (CLOUD_TILE_SHIFT / 2));
    for tile in (0..cells.clouds.len() * WORD_BITS / server::host::frame::CLOUD_BITS).filter(|&tile| cloud_over(&cells.clouds, tile) != 0) {
        let ((left, top), (x, y)) = (chunk_top_left(tile / tiles_a_chunk), cell_of_bit((tile % tiles_a_chunk) << CLOUD_TILE_SHIFT));
        shadow((left + x as usize, top + y as usize), side, cloud_over(&cells.clouds, tile), rains_over(&cells.raining, tile));
    }
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
                let (x, y) = cell_of_bit(word_index * WORD_BITS + bits.trailing_zeros() as usize);
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
            let (x, y) = cell_of_bit(index % CHUNK_WORDS * WORD_BITS + bit as usize);
            pixels[(top + y as usize) * SIDE + left + x as usize] = opaque(tree_colour(stage_at(cells, index, bit)));
            bits &= bits - 1;
        }
    }
    // Its water over the ground: the deeper, the less of the ground seen.
    for (index, &word) in cells.wet.iter().enumerate() {
        let (left, top) = chunk_top_left(index / CHUNK_WORDS);
        let mut bits = word;
        while bits != 0 {
            let bit = bits.trailing_zeros();
            let (x, y) = cell_of_bit(index % CHUNK_WORDS * WORD_BITS + bit as usize);
            let pixel = &mut pixels[(top + y as usize) * SIDE + left + x as usize];
            *pixel = opaque(under_water([pixel[0], pixel[1], pixel[2]], depth_at(cells, index, bit)));
            bits &= bits - 1;
        }
    }
    // All of it in the ground's light: the land's its slope's and its height's, the water's its depth's.
    for (pixel, &shade) in pixels.iter_mut().zip(&ground.levels[0]) {
        *pixel = opaque(lit([pixel[0], pixel[1], pixel[2]], shade));
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
    // The clouds' shadows over all of it, the sheep too.
    each_cloud(cells, |(left, top), side, cloud, raining| {
        for pixel in (top..top + side).flat_map(|y| (left..left + side).map(move |x| y * SIDE + x)) {
            let shaded = under_cloud([pixels[pixel][0], pixels[pixel][1], pixels[pixel][2]], cloud as usize, DEEPEST_CLOUD as usize);
            // Rain under a cloud deep enough: a streak on some of its cells.
            pixels[pixel] = opaque(if raining && streak_of_rain_on(pixel % SIDE, pixel / SIDE) { in_rain(shaded, 1, 1) } else { shaded });
        }
    });
    Painted { at: cells.at, side: SIDE as u32, pixels: pixels.into_flattened() }
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
fn counted(words: &[u64], detail: u32) -> Vec<u32> {
    let (side, tile_cells) = (SIDE >> detail, 1usize << (2 * detail));
    let mut counts = vec![0u32; side * side];
    for (place, chunk) in words.as_chunks::<CHUNK_WORDS>().0.iter().enumerate() {
        let (left, top) = chunk_top_left(place);
        let (left, top) = (left >> detail, top >> detail);
        for tile in 0..CELLS_IN_CHUNK / tile_cells {
            let count: u32 = if tile_cells >= WORD_BITS {
                let words = tile_cells / WORD_BITS;
                chunk[tile * words..][..words].iter().map(|word| word.count_ones()).sum()
            } else {
                let bit = tile * tile_cells;
                (chunk[bit / WORD_BITS] >> (bit % WORD_BITS) & ((1 << tile_cells) - 1)).count_ones()
            };
            let (x, y) = cell_of_bit(tile);
            counts[(top + y as usize) * side + left + x as usize] = count;
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
    let (wet, deep) = (counted(&cells.wet, detail), counted(&cells.water.deep, detail));
    let mut sheep = vec![0u16; side * side];
    for &(x, y) in &cells.sheep {
        let at = (y as usize >> detail) * side + (x as usize >> detail);
        sheep[at] = sheep[at].saturating_add(1);
    }
    // The cloud over each pixel's cells, added up: a tile's cloud once for every cell of it under the pixel.
    let mut clouds = vec![0usize; if cells.clouds.is_empty() { 0 } else { side * side }];
    // And the cells of each pixel that are under a raining cloud.
    let mut rain = clouds.clone();
    each_cloud(cells, |(left, top), tile_side, cloud, raining| {
        let (pixels_a_side, cells_each) = ((tile_side >> detail).max(1), tile_side.min(1 << detail));
        for pixel in ((top >> detail)..(top >> detail) + pixels_a_side).flat_map(|y| ((left >> detail)..(left >> detail) + pixels_a_side).map(move |x| y * side + x)) {
            clouds[pixel] += cloud as usize * cells_each * cells_each;
            if raining {
                rain[pixel] += cells_each * cells_each;
            }
        }
    });
    let sheep_cells = (2 * SHEEP_REACH + 1) * (2 * SHEEP_REACH + 1);
    let mut pixels = Vec::with_capacity(side * side * 4);
    for (index, &shade) in ground.levels[(detail as usize).min(COARSEST)].iter().enumerate() {
        let (grass, trees, sheep) = (grass[index], trees[index], sheep[index]);
        let ground = mixed(mixed(BROWN, GREEN, grass as usize, tile_cells), tree_colour(OLDEST_TREE_STAGE / 2), trees as usize, tile_cells);
        // The water over the ground, by the share of the tile under it, shallow water a quarter seen through; and all of it in the ground's light.
        let ground = lit(mixed(ground, WATER, (3 * wet[index] as usize + deep[index] as usize) / 4, tile_cells), shade);
        let pixel = mixed(ground, WHITE, sheep as usize * sheep_cells, tile_cells);
        // Under the clouds' shadow, and the rain's streaks as thin as they are among the cells.
        pixels.extend_from_slice(&opaque(clouds.get(index).map_or(pixel, |&cloud| in_rain(under_cloud(pixel, cloud, DEEPEST_CLOUD as usize * tile_cells), rain[index], CELLS_A_STREAK_OF_RAIN * tile_cells))));
    }
    Painted { at: cells.at, side: side as u32, pixels }
}
