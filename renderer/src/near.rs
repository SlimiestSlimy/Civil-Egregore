//! The world from near, a cell several pixels: one picture of the
//! viewport's cells, height drawn at its edges -- a line at a step, a
//! band under a wall (`docs/renderer.md`, "Height, from straight
//! above" and "Corners").

use crate::ground::relief::{laid, tint_on_sand, water_light, FOAM, FOAM_MOST, PALE, SAND, SAND_MOST};
use crate::ground::{shadow_drop, Fine, Ground, MARGIN, SIDE};
use crate::paint::{depth_at, stage_at, tree_colour, under_water};
use server::host::frame::{Cells, Near, DEEP, OLDEST_TREE_STAGE, WORD_BITS};
use utilities::tuning::{Tuning, RELIEF, SHADOW, STEP_DARK, STEP_LIGHT, TEXTURE, WALL_FADE, WALL_LENGTH, WALL_LIT, WALL_SHADE};
use coordinates::place_from_cartesian;
use std::collections::HashMap;
use utilities::hash::mix;
use crate::paint::{BROWN, GREEN, WHITE};

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

/// The viewport's cells from near, painted.
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
    /// How much higher the neighbour is: for a wall, than the cell at
    /// its foot.
    rise: i32,
    /// How far off the wall is, in eighths: the cells between.
    away: f32,
}

/// How long the band of a wall rising `rise` is, in eighths of a
/// cell: no longer than is looked for it.
fn band(rise: i32, tuning: &Tuning) -> f32 {
    (4.0 + rise as f32 * tuning[WALL_LENGTH]).clamp(5.0, EIGHTHS * MARGIN as f32)
}

impl Edge {
    /// What the edge does to a pixel `from` eighths in from it and
    /// `span` wide: how dark it makes it and how light, 1 neither --
    /// by as much as `tuning` says.
    fn shading(self, from: f32, span: f32, tuning: &Tuning) -> (f32, f32) {
        // The share of the pixel within a band of the edge.
        let within = |nearest: f32, farthest: f32| ((from + span).min(farthest) - from.max(nearest)).max(0.0) / span;
        // The sun is up and to the left: a neighbour down or to the right shows it its face, one up or to the left its back.
        let facing = self.towards.0 + self.towards.1;
        match self.rise {
            2.. => {
                // A wall: a band on its lower cell, darkest at its foot -- by one number if the wall faces away from the sun, by another if the sun is on it, and between the two at a corner that faces neither way.
                // The higher the wall the longer the band, over the cells before it, and no darker.
                let (width, from) = (band(self.rise, tuning), from + self.away);
                let foot = match facing {
                    ..0 => tuning[WALL_SHADE],
                    0 => (tuning[WALL_SHADE] + tuning[WALL_LIT]) / 2.0,
                    _ => tuning[WALL_LIT],
                };
                (1.0 - foot * (1.0 - tuning[WALL_FADE] * from / width).max(0.0) * within(0.0, width), 1.0)
            }
            // The border of the higher cell: light where it looks to the sun, over a lower cell up or to the left; dark where it looks away.
            ..=-1 if facing < 0 => (1.0, 1.0 + tuning[STEP_LIGHT] * within(0.0, 1.0)),
            ..=-1 if facing > 0 => (1.0 - tuning[STEP_DARK] * within(0.0, 1.0), 1.0),
            _ => (1.0, 1.0),
        }
    }
}

/// The cells `near` asks for as one picture, from `cells` -- the
/// superchunks they are in -- and their `grounds`, shaded as `tuning`
/// says. A cold superchunk, and one past the world shown, is black.
pub fn paint_near(cells: &[Cells], grounds: &HashMap<(u32, u32), Ground>, near: Near, tuning: &Tuning) -> PaintedNear {
    let pixels_a_cell = near.pixels_a_cell as usize;
    let (first, size) = ((near.first.0 as usize, near.first.1 as usize), (near.size.0 as usize, near.size.1 as usize));
    let width = size.0 * pixels_a_cell;
    let mut pixels = vec![[0, 0, 0, u8::MAX]; width * size.1 * pixels_a_cell];
    for cells in cells {
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
                let grass = cells.grass[place / WORD_BITS] >> (place % WORD_BITS) & 1 == 1;
                let (word, bit) = (place / WORD_BITS, (place % WORD_BITS) as u32);
                let water = (cells.wet[word] >> bit & 1 == 1).then(|| depth_at(cells, word, bit));
                let cell = Cell { fine, tuning, at: (own_x, own_y), world: (cells.top_left.0 as u64 + own_x as u64, cells.top_left.1 as u64 + own_y as u64), colour: if grass { GREEN } else { BROWN }, water };
                let corner = ((x - first.0) * pixels_a_cell, (y - first.1) * pixels_a_cell);
                cell.paint(&mut pixels, width, corner, pixels_a_cell);
                if cells.trees[word] >> bit & 1 == 1 {
                    tree(&mut pixels, width, corner, pixels_a_cell, stage_at(cells, word, bit));
                }
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
    /// What its shading is tuned by.
    tuning: &'a Tuning,
    /// Where it is, from its superchunk's top left.
    at: (usize, usize),
    /// Where it is in the world.
    world: (u64, u64),
    /// Its colour: dirt's or grass's.
    colour: [u8; 3],
    /// How deep the water over it is, if it is wet.
    water: Option<u32>,
}

impl Cell<'_> {
    /// Paints the cell `pixels_a_cell` a side into `pixels`, `width` a
    /// row, its top left pixel at `corner`.
    fn paint(&self, pixels: &mut [[u8; 4]], width: usize, corner: (usize, usize), pixels_a_cell: usize) {
        let (x, y) = (self.at.0 as isize, self.at.1 as isize);
        let here = self.fine.height(x, y);
        let (mut edges, mut count) = ([Edge::default(); 12], 0);
        let tuning = self.tuning;
        let rise_towards = |towards: (isize, isize)| self.fine.height(x + towards.0, y + towards.1) as i32 - here as i32;
        // The wall met first going `towards`, over ground that does not drop as a wall would: how many cells off it stands and how much it rises, if its band reaches this cell.
        let wall_towards = |towards: (isize, isize)| {
            let step = |off: isize| rise_towards((towards.0 * off, towards.1 * off)) - rise_towards((towards.0 * (off - 1), towards.1 * (off - 1)));
            let met = (1..=MARGIN as isize).find_map(|off| match step(off) {
                2.. => Some(Some(off)),
                ..=-2 => Some(None),
                _ => None,
            });
            let off = met.flatten()?;
            (((off - 1) as f32) * EIGHTHS < band(step(off), tuning)).then_some((off, step(off)))
        };
        for towards in AROUND {
            let rise = rise_towards(towards);
            let corner = towards.0 != 0 && towards.1 != 0;
            if rise <= -1 && !corner {
                edges[count] = Edge { towards, rise, away: 0.0 };
                count += 1;
            }
            let Some((off, rise)) = wall_towards(towards) else { continue };
            // A corner alone says nothing: one counts only where the cells either side of it are no higher than this one, and no wall is as near straight up or across -- where it joins their two bands.
            let nearer = |straight: (isize, isize)| rise_towards(straight) > 0 || wall_towards(straight).is_some_and(|(straight_off, _)| straight_off <= off);
            if !corner || !(nearer((towards.0, 0)) || nearer((0, towards.1))) {
                edges[count] = Edge { towards, rise, away: (off - 1) as f32 * EIGHTHS };
                count += 1;
            }
        }
        // Under deep water nothing of the ground shows, its edges neither.
        if self.water.is_some_and(|depth| depth >= DEEP) {
            count = 0;
        }
        // Its colour under its water; sand on the land beside the ocean, foam on the ocean beside the land; and what its height tints it, or its depth.
        let ocean_within = |reach: isize, ocean: bool| (-reach..=reach).flat_map(|down| (-reach..=reach).map(move |across| (across, down))).any(|(across, down)| self.fine.under_ocean(x + across, y + down).is_some() == ocean);
        let colour = self.water.map_or(self.colour, |depth| under_water(self.colour, depth));
        let (colour, tinted) = match self.fine.under_ocean(x, y) {
            Some(depth) => (if ocean_within(1, false) { laid(colour, FOAM, FOAM_MOST) } else { colour }, water_light(depth as f32, self.fine.deepest())),
            None => {
                let (tint, pale) = self.fine.tint(x, y);
                let sand = if ocean_within(1, true) { SAND_MOST } else if ocean_within(2, true) { SAND_MOST / 2.0 } else { 0.0 };
                (if sand >= pale { laid(colour, SAND, sand) } else { laid(colour, PALE, pale) }, tint_on_sand(tint, sand))
            }
        };
        // Water is smooth: its pixels do not differ by lot as the ground's do.
        let texture = if self.water.is_some() { 0.0 } else { tuning[TEXTURE] };
        // The shadow lines that reach the cell: down the diagonal, and from above and from the left.
        let (drop, over) = (shadow_drop(), self.fine.surface(x, y) as f32 + 0.01);
        let (diagonal, above, beside) = (self.fine.line(x - 1, y - 1) - over, self.fine.line(x, y - 1) - over, self.fine.line(x - 1, y) - over);
        let may_be_shadowed = diagonal.max(above).max(beside) > 0.0;
        let light = 1.0 + tuning[RELIEF] * (self.fine.light(self.at.0, self.at.1) - 1.0);
        // A cast shadow: darker, and bluer.
        let shadow = [1.0 - tuning[SHADOW], 1.0 - 0.885 * tuning[SHADOW], 1.0 - 0.46 * tuning[SHADOW]];
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
                    let (darkens, brightens) = edge.shading(from, span, tuning);
                    (dark, bright) = (dark.min(darkens), bright.max(brightens));
                }
                let shadowed = may_be_shadowed && {
                    // The pixel's middle, in cells from the cell's top left: the shadow line has dropped by how far it has come.
                    let (in_x, in_y) = ((across as f32 + 0.5) / pixels_a_cell as f32, (down as f32 + 0.5) / pixels_a_cell as f32);
                    diagonal > drop * in_x.max(in_y) || if in_x > in_y { above > drop * in_y } else { beside > drop * in_x }
                };
                // One edge's doing a pixel, a little blue as the shadows are; and under a cast shadow an edge still shows, darker than it.
                let shade = match (dark < 1.0, shadowed) {
                    (true, true) => [dark * shadow[0], dark * shadow[1], (dark * 1.08).min(1.0) * shadow[2]],
                    (true, false) => [dark, dark, (dark * 1.08).min(1.0)],
                    (false, true) => shadow.map(|shadow| shadow * bright),
                    (false, false) => [bright; 3],
                };
                let (pixel_x, pixel_y) = (self.world.0 * pixels_a_cell as u64 + across as u64, self.world.1 * pixels_a_cell as u64 + down as u64);
                // Each mixed in whole: a world pixel's place takes more than 32 bits.
                let tone = (1.0 + texture * (TONES[(mix(mix(pixel_x) ^ pixel_y) >> 60) as usize] - 1.0)) * light;
                let colour: [u8; 3] = std::array::from_fn(|channel| (colour[channel] as f32 * tone * tinted[channel] * shade[channel]).round().min(255.0) as u8);
                pixels[(corner.1 + down) * width + corner.0 + across] = [colour[0], colour[1], colour[2], u8::MAX];
            }
        }
    }
}

/// A tree `stage` old on the cell whose top left pixel is at `corner`:
/// a square in the cell's middle, larger and darker the older it is,
/// from three eighths of the cell to the whole of it.
fn tree(pixels: &mut [[u8; 4]], width: usize, corner: (usize, usize), pixels_a_cell: usize, stage: u32) {
    let colour = tree_colour(stage);
    let side = (pixels_a_cell * (3 + 5 * stage as usize / OLDEST_TREE_STAGE as usize) / 8).max(1);
    let inset = (pixels_a_cell - side) / 2;
    for down in inset..inset + side {
        for across in inset..inset + side {
            // Its lower right edge a little darker: a crown, lit from the top left.
            let shade = if side > 2 && (down + 1 == inset + side || across + 1 == inset + side) { 3 } else { 4 };
            let pixel = colour.map(|channel| (channel as usize * shade / 4) as u8);
            pixels[(corner.1 + down) * width + corner.0 + across] = [pixel[0], pixel[1], pixel[2], u8::MAX];
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
