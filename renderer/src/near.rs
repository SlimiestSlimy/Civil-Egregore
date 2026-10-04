//! The world from near, a cell several pixels: one picture of the
//! cells in view, where height is drawn at its edges -- a step of one
//! a thin line, a wall a band on its lower cell and a lip on its upper
//! -- over the ground's light ([`crate::ground`]).
//!
//! Every pixel takes one edge's doing, never two multiplied: of the
//! edges that darken it the darkest, and only if none does, of those
//! that lighten it the lightest. A cast shadow and an edge's shade are
//! joined the same way, the darker of the two. A wall met only at a
//! corner fills that corner, joining the bands either side of it; a
//! step met only at a corner draws nothing, a dot alone saying
//! nothing. So bands meet at corners as one outline, with no doubled
//! patch and no gap.

use crate::ground::{shadow_drop, Fine, Ground, SHADOW, SIDE};
use crate::sim::{Cells, Near};
use bitmap::BITS_PER_WORD;
use coordinates::place_from_cartesian;
use std::collections::HashMap;
use utilities::hash::mix;
use world::diagnostics::frames::{BROWN, GREEN, WHITE};

/// A cell's side in eighths: what edges are measured in, whatever the
/// pixels a cell.
const EIGHTHS: f32 = 8.0;

/// The eight cells about a cell: across and down.
const AROUND: [(isize, isize); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// How much lighter or darker a pixel's ground is than its cell's
/// colour, by lot: most as it is, some a little off.
const TONES: [f32; 16] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.08, 1.08, 1.08, 1.08, 0.92, 0.92, 0.92, 1.2];

/// A sheep on a cell eight pixels a side, from above.
const SHEEP: [&str; 8] = ["........", "........", "..####..", ".######.", ".######.", "..####..", "..#..#..", "........"];

/// The cells in view from near, painted.
pub struct PaintedNear {
    /// What was asked for.
    pub near: Near,
    /// Its pixels, row by row: red, green, blue, opacity.
    pub pixels: Vec<u8>,
}

/// An edge of a cell: a neighbour of another height.
#[derive(Clone, Copy, Default)]
struct Edge {
    /// Towards the neighbour: across and down.
    towards: (isize, isize),
    /// How much higher the neighbour is.
    rise: i32,
}

impl Edge {
    /// What the edge does to a pixel `from` eighths in from it and
    /// `span` wide: how dark it makes it and how light, 1 neither.
    fn shading(self, from: f32, span: f32) -> (f32, f32) {
        // The share of the pixel within a band of the edge.
        let within = |nearest: f32, farthest: f32| ((from + span).min(farthest) - from.max(nearest)).max(0.0) / span;
        // Down or to the right the neighbour's face is towards the sun; up or to the left, away.
        let facing = self.towards.0 + self.towards.1;
        match self.rise {
            2.. if facing < 0 => {
                // A wall in its own shade: a band, darkest at its foot.
                let width = (2 + self.rise / 2).clamp(3, 5) as f32;
                (1.0 + (0.45 * from / width - 0.75) * within(0.0, width), 1.0)
            }
            // A wall the sun is on: a dark foot and, facing it, a bright line.
            2.. => (1.0 - 0.55 * within(0.0, 2.0), 1.0 + if facing > 0 { 0.3 * within(2.0, 3.0) } else { 0.0 }),
            1 if facing < 0 => (1.0 - 0.16 * within(0.0, 1.0), 1.0),
            1 if facing > 0 => (1.0, 1.0 + 0.08 * within(0.0, 1.0)),
            // The lip over a wall.
            ..=-2 => (1.0, 1.0 + 0.32 * within(0.0, 1.0)),
            _ => (1.0, 1.0),
        }
    }
}

/// The cells `near` asks for as one picture, from `cells` -- the
/// superchunks they are in -- and their `grounds`. A cold superchunk,
/// and one past the world shown, is black.
pub fn paint_near(cells: &[Cells], grounds: &HashMap<(u32, u32), Ground>, near: Near) -> PaintedNear {
    let pixels_a_cell = near.pixels_a_cell as usize;
    let (first, size) = ((near.first.0 as usize, near.first.1 as usize), (near.size.0 as usize, near.size.1 as usize));
    let width = size.0 * pixels_a_cell;
    let mut pixels = vec![[0, 0, 0, u8::MAX]; width * size.1 * pixels_a_cell];
    for cells in cells.iter().filter(|cells| cells.hot) {
        let Some(fine) = grounds.get(&cells.top_left).and_then(|ground| ground.fine.as_ref()) else {
            continue;
        };
        let (left, top) = (cells.at.0 as usize * SIDE, cells.at.1 as usize * SIDE);
        // The superchunk's cells in the picture, from its own top left.
        let across = first.0.max(left)..(first.0 + size.0).min(left + SIDE);
        let down = first.1.max(top)..(first.1 + size.1).min(top + SIDE);
        for y in down.clone() {
            for x in across.clone() {
                let (own_x, own_y) = (x - left, y - top);
                let place = place_from_cartesian(own_x as u32, own_y as u32);
                let grass = cells.grass[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & 1 == 1;
                let cell = Cell { fine, at: (own_x, own_y), world: (cells.top_left.0 as u64 + own_x as u64, cells.top_left.1 as u64 + own_y as u64), colour: if grass { GREEN } else { BROWN } };
                cell.paint(&mut pixels, width, ((x - first.0) * pixels_a_cell, (y - first.1) * pixels_a_cell), pixels_a_cell);
            }
        }
        for &(x, y) in &cells.sheep {
            let (x, y) = (left + x as usize, top + y as usize);
            if across.contains(&x) && down.contains(&y) {
                sheep(&mut pixels, width, ((x - first.0) * pixels_a_cell, (y - first.1) * pixels_a_cell), pixels_a_cell);
            }
        }
    }
    PaintedNear { near, pixels: pixels.into_flattened() }
}

/// A cell to paint.
struct Cell<'a> {
    /// Its superchunk's ground.
    fine: &'a Fine,
    /// Where it is, from its superchunk's top left.
    at: (usize, usize),
    /// Where it is in the world.
    world: (u64, u64),
    /// Its colour: dirt's or grass's.
    colour: [u8; 3],
}

impl Cell<'_> {
    /// Paints the cell `pixels_a_cell` a side into `pixels`, `width` a
    /// row, its top left pixel at `corner`.
    fn paint(&self, pixels: &mut [[u8; 4]], width: usize, corner: (usize, usize), pixels_a_cell: usize) {
        let (x, y) = (self.at.0 as isize, self.at.1 as isize);
        let here = self.fine.height(x, y);
        let (mut edges, mut count) = ([Edge::default(); 8], 0);
        let rise_towards = |towards: (isize, isize)| self.fine.height(x + towards.0, y + towards.1) as i32 - here as i32;
        for towards in AROUND {
            let rise = rise_towards(towards);
            // A corner alone says nothing: one counts only for a wall, and only where the cells either side of it are no higher than this one -- where it joins their two bands.
            let corner = towards.0 != 0 && towards.1 != 0;
            let joins = rise.abs() >= 2 && rise_towards((towards.0, 0)) <= 0 && rise_towards((0, towards.1)) <= 0;
            if (rise >= 1 || rise <= -2) && (!corner || joins) {
                edges[count] = Edge { towards, rise };
                count += 1;
            }
        }
        // The shadow lines that reach the cell: down the diagonal, and from above and from the left.
        let (drop, over) = (shadow_drop(), here as f32 + 0.01);
        let (diagonal, above, beside) = (self.fine.line(x - 1, y - 1) - over, self.fine.line(x, y - 1) - over, self.fine.line(x - 1, y) - over);
        let may_be_shadowed = diagonal.max(above).max(beside) > 0.0;
        let light = self.fine.light(self.at.0, self.at.1);
        let span = EIGHTHS / pixels_a_cell as f32;
        for down in 0..pixels_a_cell {
            for across in 0..pixels_a_cell {
                // How far in the pixel is from the cell's left, right, top and bottom, in eighths.
                let (left, top) = (across as f32 * span, down as f32 * span);
                let (right, bottom) = (EIGHTHS - span - left, EIGHTHS - span - top);
                let (mut dark, mut bright) = (1.0f32, 1.0f32);
                for edge in &edges[..count] {
                    let from = match edge.towards {
                        (-1, 0) => left,
                        (1, 0) => right,
                        (0, -1) => top,
                        (0, 1) => bottom,
                        // A corner's: within its square only.
                        (-1, -1) => left.max(top),
                        (1, -1) => right.max(top),
                        (-1, 1) => left.max(bottom),
                        _ => right.max(bottom),
                    };
                    let (darkens, brightens) = edge.shading(from, span);
                    (dark, bright) = (dark.min(darkens), bright.max(brightens));
                }
                let shadowed = may_be_shadowed && {
                    // The pixel's middle, in cells from the cell's top left: the shadow line has dropped by how far it has come.
                    let (in_x, in_y) = ((across as f32 + 0.5) / pixels_a_cell as f32, (down as f32 + 0.5) / pixels_a_cell as f32);
                    diagonal > drop * in_x.max(in_y) || if in_x > in_y { above > drop * in_y } else { beside > drop * in_x }
                };
                // One doing a pixel: an edge's shade a little blue, as the shadows are.
                let shade = match (dark < 1.0, shadowed) {
                    (true, true) => [dark.min(SHADOW[0]), dark.min(SHADOW[1]), (dark * 1.08).min(SHADOW[2])],
                    (true, false) => [dark, dark, (dark * 1.08).min(1.0)],
                    (false, true) => SHADOW.map(|shadow| shadow * bright),
                    (false, false) => [bright; 3],
                };
                let (pixel_x, pixel_y) = (self.world.0 * pixels_a_cell as u64 + across as u64, self.world.1 * pixels_a_cell as u64 + down as u64);
                let tone = TONES[(mix(pixel_x << 32 | pixel_y) >> 60) as usize] * light;
                let colour: [u8; 3] = std::array::from_fn(|channel| (self.colour[channel] as f32 * tone * shade[channel]).round().min(255.0) as u8);
                pixels[(corner.1 + down) * width + corner.0 + across] = [colour[0], colour[1], colour[2], u8::MAX];
            }
        }
    }
}

/// A sheep on the cell whose top left pixel is at `corner`: its shape,
/// drawn as coarsely as the cell is.
fn sheep(pixels: &mut [[u8; 4]], width: usize, corner: (usize, usize), pixels_a_cell: usize) {
    let coarser = SHEEP.len() / pixels_a_cell;
    for down in 0..pixels_a_cell {
        for across in 0..pixels_a_cell {
            // White where a quarter of what is under the pixel is the sheep's.
            let white = (0..coarser * coarser).filter(|index| SHEEP[down * coarser + index / coarser].as_bytes()[across * coarser + index % coarser] == b'#').count();
            if 4 * white >= coarser * coarser {
                pixels[(corner.1 + down) * width + corner.0 + across] = [WHITE[0], WHITE[1], WHITE[2], u8::MAX];
            }
        }
    }
}
