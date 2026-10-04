//! Height, drawn top down: one patch of generated terrain rendered with
//! each artistic layer switched on in turn, plus a mid and a far view.
//!
//! `heightart [seed] [metres a height unit] [sun elevation, degrees] [out folder]`

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// A cell's side, in metres.
const CELL_METRES: f32 = 2.0;
/// Pixels a cell side, close up.
const PX: usize = 8;
/// The world's middle, in cells.
const MIDDLE: u32 = 1 << 31;
/// Cells of terrain beyond the top left kept for shadows cast into the view.
const SHADOW_MARGIN: usize = 192;
/// Cells beyond every edge kept for smoothing and neighbours.
const EDGE: usize = 16;

/// Heights over a rectangle of cells.
struct Field {
    x0: u32,
    y0: u32,
    w: usize,
    h: usize,
    data: Vec<u8>,
}

impl Field {
    fn generate(seed: u64, x0: u32, y0: u32, w: usize, h: usize) -> Self {
        let mut data = vec![0; w * h];
        std::thread::scope(|scope| {
            for (band, rows) in data.chunks_mut(w * h.div_ceil(16)).enumerate() {
                scope.spawn(move || {
                    let first = band * h.div_ceil(16);
                    for (i, cell) in rows.iter_mut().enumerate() {
                        let (x, y) = (i % w, first + i / w);
                        *cell = terrain::height(seed, x0 + x as u32, y0 + y as u32);
                    }
                });
            }
        });
        Self { x0, y0, w, h, data }
    }
    fn at(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.w + x]
    }
    /// Heights averaged over `block` x `block` cells: a coarser field.
    fn coarse(&self, block: usize) -> FieldF {
        let (w, h) = (self.w / block, self.h / block);
        let mut data = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0u32;
                for dy in 0..block {
                    for dx in 0..block {
                        sum += self.at(x * block + dx, y * block + dy) as u32;
                    }
                }
                data[y * w + x] = sum as f32 / (block * block) as f32;
            }
        }
        FieldF { w, h, data }
    }
    fn float(&self) -> FieldF {
        FieldF { w: self.w, h: self.h, data: self.data.iter().map(|&v| v as f32).collect() }
    }
}

/// Heights as floats, for smoothing.
#[derive(Clone)]
struct FieldF {
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl FieldF {
    fn at(&self, x: usize, y: usize) -> f32 {
        self.data[y.min(self.h - 1) * self.w + x.min(self.w - 1)]
    }
    /// Box blur of radius `r`, twice: near enough a gaussian.
    fn blurred(&self, r: usize) -> Self {
        let mut out = self.clone();
        for _ in 0..2 {
            for pass in 0..2 {
                let src = out.clone();
                for y in 0..self.h {
                    for x in 0..self.w {
                        let mut sum = 0.0;
                        for d in -(r as isize)..=(r as isize) {
                            let (sx, sy) = if pass == 0 { ((x as isize + d).clamp(0, self.w as isize - 1) as usize, y) } else { (x, (y as isize + d).clamp(0, self.h as isize - 1) as usize) };
                            sum += src.data[sy * self.w + sx];
                        }
                        out.data[y * self.w + x] = sum / (2 * r + 1) as f32;
                    }
                }
            }
        }
        out
    }
}

/// The light: from the top left, `elevation` degrees up.
#[derive(Clone, Copy)]
struct Light {
    /// Unit vector toward the sun; x right, y down, z up.
    to_sun: [f32; 3],
    /// Height units the shadow line drops over one diagonal step of `side` metres.
    tan: f32,
}

impl Light {
    fn new(elevation: f32) -> Self {
        let e = elevation.to_radians();
        let flat = e.cos() / 2f32.sqrt();
        Self { to_sun: [-flat, -flat, e.sin()], tan: e.tan() }
    }
    /// Lambert over flat ground's: 1 flat, above 1 facing the sun.
    fn shade(&self, dzdx: f32, dzdy: f32) -> f32 {
        let n = [-dzdx, -dzdy, 1.0];
        let len = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
        let lit = (n[0] * self.to_sun[0] + n[1] * self.to_sun[1] + n[2] * self.to_sun[2]) / len;
        lit.max(0.0) / self.to_sun[2]
    }
}

/// Which layers to draw.
#[derive(Clone, Copy, Default)]
struct Layers {
    steps: bool,
    walls: bool,
    hillshade: bool,
    tint: bool,
    shadows: bool,
    people: bool,
}

/// Settings shared by every render.
struct Style {
    unit: f32,
    light: Light,
}

fn hash(a: u64, b: u64) -> u64 {
    let mut z = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z ^= z >> 31;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^ (z >> 29)
}

/// The grass texture's colour at world pixel `(x, y)`.
fn grass(x: u64, y: u64) -> [f32; 3] {
    const TONES: [[f32; 3]; 4] = [[92.0, 146.0, 62.0], [100.0, 156.0, 66.0], [84.0, 134.0, 58.0], [112.0, 166.0, 74.0]];
    let r = hash(x, y) % 16;
    TONES[match r {
        0..=7 => 0,
        8..=11 => 1,
        12..=14 => 2,
        _ => 3,
    }]
}

/// Bands `v` to steps of `step` about 1: pixel-art shading, not airbrushed.
fn band(v: f32, step: f32) -> f32 {
    1.0 + ((v - 1.0) / step).round() * step
}

/// Altitude tint: low ground a little darker, high a little lighter.
fn tint(h: f32) -> f32 {
    0.86 + 0.26 * (h / 255.0)
}

/// Cast shadows over a grid of `w` x `h` samples of `side` metres, heights
/// from `height`: true where shadowed. A sweep along the light's diagonal.
fn shadows(w: usize, h: usize, side: f32, style: &Style, height: impl Fn(usize, usize) -> f32) -> Vec<bool> {
    let drop = side * 2f32.sqrt() * style.light.tan / style.unit;
    let mut line = vec![f32::MIN; w * h];
    let mut shadowed = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let here = height(x, y);
            let from = if x > 0 && y > 0 { line[(y - 1) * w + x - 1] - drop } else { f32::MIN };
            shadowed[y * w + x] = from > here + 0.01;
            line[y * w + x] = from.max(here);
        }
    }
    shadowed
}

/// Four neighbours: (dx, dy, faces the sun) -- a rise toward the bottom
/// right faces the sun at the top left.
const SIDES: [(isize, isize, bool); 4] = [(-1, 0, false), (0, -1, false), (1, 0, true), (0, 1, true)];

struct Image {
    w: usize,
    h: usize,
    rgb: Vec<[f32; 3]>,
}

impl Image {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, rgb: vec![[0.0; 3]; w * h] }
    }
    fn scale(&mut self, x: usize, y: usize, by: [f32; 3]) {
        let p = &mut self.rgb[y * self.w + x];
        for c in 0..3 {
            p[c] *= by[c];
        }
    }
    fn set(&mut self, x: usize, y: usize, to: [f32; 3]) {
        self.rgb[y * self.w + x] = to;
    }
    fn save(&self, path: &Path) {
        let file = BufWriter::new(File::create(path).expect("the png"));
        let mut encoder = png::Encoder::new(file, self.w as u32, self.h as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        let bytes: Vec<u8> = self.rgb.iter().flat_map(|p| p.map(|c| c.round().clamp(0.0, 255.0) as u8)).collect();
        writer.write_image_data(&bytes).expect("pixels");
    }
}

const SHADOW: [f32; 3] = [0.74, 0.77, 0.88];

/// The patch of `field` at cells `(cx, cy)`, `cw` x `ch`, close up: a cell 8 px.
fn close(field: &Field, smooth: &FieldF, cx: usize, cy: usize, cw: usize, ch: usize, layers: Layers, style: &Style) -> Image {
    let mut img = Image::new(cw * PX, ch * PX);
    // Shadows in pixels, from the margin above and left of the patch.
    let (sx, sy) = (cx - SHADOW_MARGIN, cy - SHADOW_MARGIN);
    let (sw, sh) = ((cw + SHADOW_MARGIN) * PX, (ch + SHADOW_MARGIN) * PX);
    let shadow = if layers.shadows { shadows(sw, sh, CELL_METRES / PX as f32, style, |x, y| field.at(sx + x / PX, sy + y / PX) as f32) } else { Vec::new() };
    for cy_ in 0..ch {
        for cx_ in 0..cw {
            let (x, y) = (cx + cx_, cy + cy_);
            let height = field.at(x, y);
            let mut factor = 1.0;
            if layers.hillshade {
                let dzdx = (smooth.at(x + 1, y) - smooth.at(x - 1, y)) / 2.0 * style.unit / CELL_METRES;
                let dzdy = (smooth.at(x, y + 1) - smooth.at(x, y - 1)) / 2.0 * style.unit / CELL_METRES;
                factor *= band(1.0 + 0.9 * (style.light.shade(dzdx, dzdy) - 1.0), 0.07).clamp(0.55, 1.35);
            }
            if layers.tint {
                factor *= tint(height as f32);
            }
            for py in 0..PX {
                for px in 0..PX {
                    let (ix, iy) = (cx_ * PX + px, cy_ * PX + py);
                    let wx = (field.x0 as u64 + x as u64) * PX as u64 + px as u64;
                    let wy = (field.y0 as u64 + y as u64) * PX as u64 + py as u64;
                    img.set(ix, iy, grass(wx, wy).map(|c| c * factor));
                    if layers.shadows && shadow[(iy + SHADOW_MARGIN * PX) * sw + ix + SHADOW_MARGIN * PX] {
                        img.scale(ix, iy, SHADOW);
                    }
                }
            }
            // Edges: steps of one, and walls, drawn on the lower cell; a lip on the upper.
            for &(dx, dy, faces_sun) in &SIDES {
                let (nx, ny) = ((x as isize + dx) as usize, (y as isize + dy) as usize);
                let rise = field.at(nx, ny) as i32 - height as i32;
                let edge = |d: usize, along: usize| -> (usize, usize) {
                    // Pixel `d` in from the edge toward (dx, dy), `along` it.
                    match (dx, dy) {
                        (-1, 0) => (d, along),
                        (1, 0) => (PX - 1 - d, along),
                        (0, -1) => (along, d),
                        _ => (along, PX - 1 - d),
                    }
                };
                if rise == 1 && layers.steps {
                    for along in 0..PX {
                        let (px, py) = edge(0, along);
                        img.scale(cx_ * PX + px, cy_ * PX + py, if faces_sun { [1.08; 3] } else { [0.84; 3] });
                    }
                }
                if rise >= 2 && layers.walls {
                    let width = (1 + rise as usize / 2).clamp(2, 4);
                    for d in 0..width {
                        for along in 0..PX {
                            let (px, py) = edge(d, along);
                            let by = if faces_sun {
                                if d == 0 { [0.62; 3] } else if d == 1 { [1.22; 3] } else { [1.0; 3] }
                            } else {
                                let k = 0.42 + 0.5 * d as f32 / width as f32;
                                [k, k, k * 1.08]
                            };
                            img.scale(cx_ * PX + px, cy_ * PX + py, by);
                        }
                    }
                }
                if rise <= -2 && layers.walls {
                    // The lip at the top of a cliff.
                    for along in 0..PX {
                        let (px, py) = edge(0, along);
                        img.scale(cx_ * PX + px, cy_ * PX + py, [1.18; 3]);
                    }
                }
            }
        }
    }
    if layers.people {
        draw_people(&mut img, cw, ch);
    }
    img
}

/// A few 7 px stickmen and a sheep or two, to judge what reads.
fn draw_people(img: &mut Image, cw: usize, ch: usize) {
    const MAN: [&str; 7] = ["..#..", "#####", "..#..", "..#..", ".#.#.", ".#.#.", "#...#"];
    const SHEEP: [&str; 5] = ["......", ".####.", "######", ".####.", ".#..#."];
    for i in 0..14u64 {
        let (x, y) = ((hash(i, 1) % cw as u64) as usize, (hash(i, 2) % ch as u64) as usize);
        let (glyph, colour, ox, oy): (&[&str], [f32; 3], usize, usize) = if i % 3 == 0 { (&SHEEP, [236.0, 234.0, 222.0], 1, 2) } else { (&MAN, [30.0, 26.0, 34.0], 1, 0) };
        for (gy, row) in glyph.iter().enumerate() {
            for (gx, c) in row.bytes().enumerate() {
                if c == b'#' {
                    img.set(x * PX + ox + gx, y * PX + oy + gy, colour);
                }
            }
        }
    }
}

/// A wide area at `block` cells a pixel: hillshade, tint, shadows,
/// contours every `contour` levels, cliffs as dark pixels.
fn far(field: &Field, block: usize, contour: u8, style: &Style) -> Image {
    let coarse = field.coarse(block);
    let smooth = coarse.blurred(if block == 1 { 4 } else { 2 });
    let (w, h) = (coarse.w, coarse.h);
    let side = CELL_METRES * block as f32;
    let shadow = shadows(w, h, side, style, |x, y| coarse.at(x, y));
    let mut img = Image::new(w, h);
    let base = [96.0, 150.0, 64.0];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let dzdx = (smooth.at(x + 1, y) - smooth.at(x - 1, y)) / 2.0 * style.unit / side;
            let dzdy = (smooth.at(x, y + 1) - smooth.at(x, y - 1)) / 2.0 * style.unit / side;
            let mut factor = band(1.0 + 0.9 * (style.light.shade(dzdx, dzdy) - 1.0), 0.05).clamp(0.5, 1.4) * tint(coarse.at(x, y));
            let level = |x: usize, y: usize| (coarse.at(x, y) / contour as f32) as i32;
            if level(x, y) != level(x + 1, y) || level(x, y) != level(x, y + 1) {
                factor *= 0.86;
            }
            img.set(x, y, base.map(|c| c * factor));
            if shadow[y * w + x] {
                img.scale(x, y, SHADOW);
            }
            // Cliffs: any wall within the block.
            let mut walls = 0;
            for dy in 0..block {
                for dx in 0..block {
                    let (cx, cy) = (x * block + dx, y * block + dy);
                    if cx + 1 < field.w && cy + 1 < field.h {
                        let here = field.at(cx, cy);
                        walls += terrain::wall(here, field.at(cx + 1, cy)) as u32 + terrain::wall(here, field.at(cx, cy + 1)) as u32;
                    }
                }
            }
            if walls > 0 {
                let k = (0.75 - 0.08 * walls.min(4) as f32).max(0.45);
                img.scale(x, y, [k; 3]);
            }
        }
    }
    img
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(4);
    let unit: f32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1.0);
    let elevation: f32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(35.0);
    let out = PathBuf::from(args.get(4).cloned().unwrap_or_else(|| "target/renders".into()));
    std::fs::create_dir_all(&out).expect("the out folder");
    let style = Style { unit, light: Light::new(elevation) };

    // The superchunk at the middle, and around it enough for shadows and edges.
    let start = MIDDLE - (SHADOW_MARGIN + EDGE) as u32;
    let side = 1024 + 2 * (SHADOW_MARGIN + EDGE);
    let field = Field::generate(seed, start, start, side, side);
    let smooth = field.float().blurred(4);

    // The 128 x 96 cell patch with the most walls in it.
    let (cw, ch) = (128, 96);
    let lo = SHADOW_MARGIN + EDGE;
    let mut best = (0, lo, lo);
    for y in (lo..lo + 1024 - ch).step_by(16) {
        for x in (lo..lo + 1024 - cw).step_by(16) {
            let mut walls = 0;
            for yy in y..y + ch {
                for xx in x..x + cw {
                    walls += terrain::wall(field.at(xx, yy), field.at(xx + 1, yy)) as u32 + terrain::wall(field.at(xx, yy), field.at(xx, yy + 1)) as u32;
                }
            }
            best = best.max((walls, x, y));
        }
    }
    let (walls, cx, cy) = best;
    let range = (cy..cy + ch).flat_map(|y| (cx..cx + cw).map(move |x| (x, y))).map(|(x, y)| field.at(x, y));
    let (min, max) = range.fold((255, 0), |(lo, hi), h| (lo.min(h), hi.max(h)));
    println!("patch at cell ({}, {}) of the middle superchunk: {walls} walls, heights {min}..={max}", cx - lo, cy - lo);

    let steps = [
        ("0_texture", Layers { people: true, ..Default::default() }),
        ("1_steps", Layers { steps: true, people: true, ..Default::default() }),
        ("2_walls", Layers { steps: true, walls: true, people: true, ..Default::default() }),
        ("3_hillshade", Layers { steps: true, walls: true, hillshade: true, people: true, ..Default::default() }),
        ("4_tint", Layers { steps: true, walls: true, hillshade: true, tint: true, people: true, ..Default::default() }),
        ("5_shadows", Layers { steps: true, walls: true, hillshade: true, tint: true, shadows: true, people: true }),
        ("6_shadows_only", Layers { walls: true, shadows: true, people: true, ..Default::default() }),
    ];
    for (name, layers) in steps {
        close(&field, &smooth, cx, cy, cw, ch, layers, &style).save(&out.join(format!("{name}.png")));
    }
    // Mid: the whole superchunk, a cell a pixel. Far: 3x3 superchunks, 4 cells a pixel.
    let mid = Field::generate(seed, MIDDLE - 64, MIDDLE - 64, 1024 + 128, 1024 + 128);
    far(&mid, 1, 8, &style).save(&out.join("7_mid_1_cell_a_pixel.png"));
    let wide = Field::generate(seed, MIDDLE - 1024 - 256, MIDDLE - 1024 - 256, 3072 + 512, 3072 + 512);
    far(&wide, 4, 16, &style).save(&out.join("8_far_4_cells_a_pixel.png"));
    println!("written to {}", out.display());
}
