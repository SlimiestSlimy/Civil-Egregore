//! The map: the world from farther than its cells are drawn from, a
//! pixel of the screen 32 cells or more a side. It asks nothing of the
//! simulation: a pixel is the cell in its middle, as that cell is
//! generated -- its height from the seed, the ocean over it or grass,
//! dirt or a tree on it -- so nothing is made hot to be looked at, and
//! the world is seen as far out as islands are specks. A thread of its
//! own draws the last map asked for, on every thread the machine has.

use crate::paint::{tree_colour, WATER};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use worldgen::mesh::Lands;
use crate::paint::{BROWN, GREEN};
use world::Generation;

/// How much of its light the deepest ocean keeps.
const DEEP_LIGHT: f32 = 0.35;
/// How much of its light a pixel on a line of the mesh keeps.
const BORDER_LIGHT: f32 = 0.25;
/// How much lighter or darker a slope of one height a cell is drawn.
const SLOPE_LIGHT: f32 = 2.5;

/// A map asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wanted {
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size: (u32, u32),
    /// The seed of the world it is of.
    pub seed: u64,
    /// How that world is generated.
    pub generation: Generation,
    /// Whether the mesh's lines are drawn over it.
    pub borders: bool,
}

/// A map drawn.
pub struct Drawn {
    /// What was asked for.
    pub wanted: Wanted,
    /// Its pixels, red, green, blue and opacity, row by row.
    pub pixels: Vec<u8>,
}

/// Starts the map's thread: where to ask for a map, and where it
/// comes back. Of several asked for while one is drawn, only the last
/// is drawn next.
pub fn start() -> (Sender<Wanted>, Receiver<Drawn>) {
    let (requests, asked) = channel::<Wanted>();
    let (answers, maps) = channel();
    thread::Builder::new()
        .name("map".to_string())
        .spawn(move || {
            while let Ok(first) = asked.recv() {
                let wanted = asked.try_iter().last().unwrap_or(first);
                if answers.send(Drawn { wanted, pixels: draw(&wanted) }).is_err() {
                    return;
                }
            }
        })
        .expect("a thread for the map");
    (requests, maps)
}

/// The cell in the middle of the pixel `(x, y)` of `wanted`, or `None`
/// past the world's edges.
fn cell(wanted: &Wanted, x: i64, y: i64) -> Option<(u32, u32)> {
    let cell = |first: i64, pixel: i64| u32::try_from(first + pixel * wanted.step as i64 + wanted.step as i64 / 2).ok();
    Some((cell(wanted.first.0, x)?, cell(wanted.first.1, y)?))
}

/// The pixels of `wanted`: rows shared out among the machine's threads.
fn draw(wanted: &Wanted) -> Vec<u8> {
    let (width, generation, seed) = (wanted.size.0 as usize, &wanted.generation, wanted.seed);
    let trees_seed = seed ^ world::TREES_SALT;
    let (grass_under, trees_under) = (generation.grass.threshold(seed), generation.trees.threshold(trees_seed));
    let mut pixels = vec![0u8; width * wanted.size.1 as usize * 4];
    let threads = thread::available_parallelism().map_or(1, |threads| threads.get());
    let rows_each = (wanted.size.1 as usize).div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (part, rows) in pixels.chunks_mut(rows_each * width * 4).enumerate() {
            scope.spawn(move || {
                let mut lands = Lands::new(&generation.shape, seed);
                for (row, pixels) in rows.chunks_mut(width * 4).enumerate() {
                    let y = (part * rows_each + row) as i64;
                    // The height up and to the left of each pixel, the sun's side: what its slope is told by.
                    let mut before = None;
                    for (x, pixel) in pixels.chunks_mut(4).enumerate() {
                        let Some((cell_x, cell_y)) = cell(wanted, x as i64, y) else {
                            pixel.copy_from_slice(&[0, 0, 0, u8::MAX]);
                            before = None;
                            continue;
                        };
                        let high = lands.height(cell_x, cell_y);
                        let shape = &generation.shape;
                        let (colour, light) = if high < shape.ocean {
                            (WATER, 1.0 - (1.0 - DEEP_LIGHT) * ((shape.ocean - high) as f32 / (shape.ocean - shape.ground).max(1) as f32).min(1.0))
                        } else {
                            let colour = if generation.trees.number(trees_seed, cell_x, cell_y) < trees_under {
                                tree_colour(8)
                            } else if generation.grass.number(seed, cell_x, cell_y) < grass_under {
                                GREEN
                            } else {
                                BROWN
                            };
                            let above = cell(wanted, x as i64, y - 1).map_or(high, |(x, y)| lands.height(x, y));
                            let lower = (above as f32 + before.unwrap_or(high) as f32) / 2.0;
                            let slope = (high as f32 - lower) / wanted.step as f32;
                            let tint = 0.8 + 0.3 * (high.saturating_sub(shape.ocean) as f32 / shape.highest.saturating_sub(shape.ocean).max(1) as f32).min(1.0);
                            (colour, tint * (1.0 + SLOPE_LIGHT * slope).clamp(0.55, 1.45))
                        };
                        before = Some(high);
                        // A line of the mesh, where the pixel is no farther from it than it is across.
                        let on_border = wanted.borders && lands.line(cell_x, cell_y).1 < wanted.step as u64;
                        let light = if on_border { light * BORDER_LIGHT } else { light };
                        let lit = colour.map(|channel| (channel as f32 * light).min(255.0) as u8);
                        pixel.copy_from_slice(&[lit[0], lit[1], lit[2], u8::MAX]);
                    }
                }
            });
        }
    });
    pixels
}
