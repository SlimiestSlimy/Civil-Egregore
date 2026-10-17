//! Bitmaps drawn with lines: straight strokes of random length, run
//! horizontally, vertically or along either diagonal, at random widths
//! -- a width of one most often, each wider one less likely, as the line
//! set says. Like every corpus bitmap, settled by a seed.

use utilities::rng::RandomSource;
use bitmap::{Bitmap, WIDTH};

/// A kind of drawing: how many lines a bitmap is given, how wide they
/// get, and how many to measure over and to test.
pub struct LineSet {
    /// What a measurement calls it.
    pub display_name: &'static str,
    /// How many lines each bitmap is given.
    pub lines: u64,
    /// The widest a line is drawn, in cells.
    pub widest_line: i64,
    /// How likely a line is to be one wider again, in a hundred: at 50,
    /// each width is half as likely as the one before.
    pub wider_in_a_hundred: u64,
    /// How many to measure over.
    pub timed_count: u64,
    /// How many a test takes.
    pub tested_count: u64,
}

impl LineSet {
    /// `count` bitmaps of this set grown from `seed`, built one at a time.
    pub fn take_bitmaps(&'static self, seed: u64, count: u64) -> LineDrawings {
        LineDrawings { seed, bitmaps_left: count, line_set: self }
    }

    /// As many as a timed run of this set should take.
    pub fn timed_bitmaps(&'static self, seed: u64) -> LineDrawings {
        self.take_bitmaps(seed, self.timed_count)
    }

    /// As many as a unit test of this set should take.
    pub fn tested_bitmaps(&'static self, seed: u64) -> LineDrawings {
        self.take_bitmaps(seed, self.tested_count)
    }
}

/// From a few strokes to a tangle.
pub const LINE_SETS: [LineSet; 3] = [
    LineSet { display_name: "a few lines", lines: 8, widest_line: 16, wider_in_a_hundred: 50, timed_count: 12, tested_count: 2 },
    LineSet { display_name: "some lines", lines: 32, widest_line: 16, wider_in_a_hundred: 50, timed_count: 12, tested_count: 2 },
    LineSet { display_name: "many lines", lines: 128, widest_line: 16, wider_in_a_hundred: 50, timed_count: 12, tested_count: 2 },
];

/// The four ways a line runs: across, down, and down either diagonal.
const LINE_ORIENTATIONS: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

/// A run of drawings from consecutive seeds, built one at a time.
pub struct LineDrawings {
    /// The next drawing's seed.
    seed: u64,
    /// How many drawings are still to come.
    bitmaps_left: u64,
    /// What every drawing is drawn by.
    line_set: &'static LineSet,
}

impl Iterator for LineDrawings {
    type Item = Bitmap;

    fn next(&mut self) -> Option<Bitmap> {
        if self.bitmaps_left == 0 {
            return None;
        }
        self.bitmaps_left -= 1;
        self.seed += 1;
        Some(one_drawn(self.seed - 1, self.line_set))
    }
}

/// One drawing: `line_set.lines` lines, each from a random cell, in a random
/// orientation, of a random length up to the bitmap's side, at a random
/// width. The border cuts what runs past it.
pub fn one_drawn(seed: u64, line_set: &LineSet) -> Bitmap {
    let mut bitmap = Bitmap::new();
    let mut random_generator = RandomSource::new(seed);
    let bitmap_side = WIDTH as u64;
    for _ in 0..line_set.lines {
        let (x, y) = (random_generator.draw_below(bitmap_side) as i64, random_generator.draw_below(bitmap_side) as i64);
        let (dx, dy) = LINE_ORIENTATIONS[random_generator.draw_below(LINE_ORIENTATIONS.len() as u64) as usize];
        let line_length = 1 + random_generator.draw_below(bitmap_side) as i64;
        let mut width = 1;
        while width < line_set.widest_line && random_generator.percent_chance(line_set.wider_in_a_hundred) {
            width += 1;
        }
        for step in 0..line_length {
            let (step_x, step_y) = (x + dx * step, y + dy * step);
            bitmap.set_rect(step_x, step_y, step_x + width - 1, step_y + width - 1);
        }
    }
    bitmap
}
