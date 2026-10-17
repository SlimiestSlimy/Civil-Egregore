//! The one source of corpus bitmaps, tests and measurements alike:
//! grown, sparse, laid out and drawn, each settled by its seed and its
//! generator's parameters, nothing stored.
//!
//! How they are used: `docs/testing_protocol.md`. Function by
//! function: `docs/lab.md`, "`corpus/`".

pub mod checkerboards;
mod city;
mod generate;
mod lines;

pub use city::{one_laid_out, CityBitmaps, CityPlan, CITY_PLANS};
pub use lines::{one_drawn, LineDrawings, LineSet, LINE_SETS};

use bitmap::Bitmap;

/// One shape worth measuring on: what it looks like, the two numbers
/// that make it, and how many of it a timed run should take.
pub struct GrownShape {
    /// What a measurement calls it.
    pub display_name: &'static str,
    /// The share of the cells that end up set.
    pub cell_density: f64,
    /// How often a new cell lands beside one already set rather than
    /// anywhere at all.
    pub cluster_weight: f64,
    /// How many to time over. Not the same for every shape, and it
    /// cannot be: a dense ragged bitmap runs to ten thousand rectangles
    /// and costs a thousand times what a sparse one does, so one count
    /// would be either too few to average or too slow to finish.
    pub timed_count: u64,
    /// How many to check in a unit test, where the budget is a second
    /// rather than a minute.
    pub tested_count: u64,
}

impl GrownShape {
    /// `count` bitmaps of this shape grown from `seed`, built one at a time.
    pub fn take_bitmaps(&self, seed: u64, count: u64) -> GrownBitmaps {
        grown_bitmaps(seed, self.cell_density, self.cluster_weight, count)
    }

    /// As many as a timed run of this shape should take.
    pub fn timed_bitmaps(&self, seed: u64) -> GrownBitmaps {
        self.take_bitmaps(seed, self.timed_count)
    }

    /// As many as a unit test of this shape should take.
    pub fn tested_bitmaps(&self, seed: u64) -> GrownBitmaps {
        self.take_bitmaps(seed, self.tested_count)
    }
}

/// The grown shapes worth measuring on, named for what they look like:
/// density and cluster weight between them span scattered cells, solid
/// blobs and the ragged middle.
pub const GROWN_SHAPES: [GrownShape; 9] = [
    GrownShape { display_name: "sparse scattered", cell_density: 0.05, cluster_weight: 0.00, timed_count: 20, tested_count: 2 },
    GrownShape { display_name: "sparse ragged", cell_density: 0.05, cluster_weight: 0.70, timed_count: 20, tested_count: 2 },
    GrownShape { display_name: "sparse blobs", cell_density: 0.05, cluster_weight: 0.95, timed_count: 20, tested_count: 2 },
    GrownShape { display_name: "middling scattered", cell_density: 0.20, cluster_weight: 0.00, timed_count: 6, tested_count: 1 },
    GrownShape { display_name: "middling ragged", cell_density: 0.20, cluster_weight: 0.70, timed_count: 6, tested_count: 1 },
    GrownShape { display_name: "middling blobs", cell_density: 0.20, cluster_weight: 0.95, timed_count: 6, tested_count: 1 },
    GrownShape { display_name: "dense scattered", cell_density: 0.50, cluster_weight: 0.00, timed_count: 2, tested_count: 1 },
    GrownShape { display_name: "dense ragged", cell_density: 0.50, cluster_weight: 0.70, timed_count: 2, tested_count: 1 },
    GrownShape { display_name: "dense blobs", cell_density: 0.50, cluster_weight: 0.95, timed_count: 2, tested_count: 1 },
];

/// Sparse bitmaps: a handful of cells to a hundredth of them, grown the
/// same way, where almost everything is clear and each set cell is what
/// the bits are spent on.
pub const SPARSE_SHAPES: [GrownShape; 11] = [
    GrownShape { display_name: "a cell or two", cell_density: 0.00002, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "a few bunched cells", cell_density: 0.0001, cluster_weight: 0.95, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "a few cells", cell_density: 0.0002, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "a tenth of a percent", cell_density: 0.001, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "a tenth of a percent, bunched", cell_density: 0.001, cluster_weight: 0.95, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "a hundred cells", cell_density: 0.0015, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "sparse clusters", cell_density: 0.005, cluster_weight: 0.70, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "one percent", cell_density: 0.01, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "one percent, bunched", cell_density: 0.01, cluster_weight: 0.95, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "two percent", cell_density: 0.02, cluster_weight: 0.00, timed_count: 12, tested_count: 2 },
    GrownShape { display_name: "two percent, bunched", cell_density: 0.02, cluster_weight: 0.95, timed_count: 12, tested_count: 2 },
];

/// A run of grown bitmaps from consecutive seeds, built one at a time,
/// so a caller measuring thousands never holds them all.
pub struct GrownBitmaps {
    /// The next bitmap's seed.
    seed: u64,
    /// How many bitmaps are still to come.
    bitmaps_left: u64,
    /// The share of the cells each bitmap ends up with set.
    cell_density: f64,
    /// How often a new cell lands beside one already set rather than
    /// anywhere at all.
    cluster_weight: f64,
}

/// `count` bitmaps from seeds `seed`, `seed + 1`, and so on, each
/// grown to `cell_density` at `cluster_weight` ([`generate::grow_one_bitmap`]).
pub fn grown_bitmaps(seed: u64, cell_density: f64, cluster_weight: f64, count: u64) -> GrownBitmaps {
    GrownBitmaps { seed, bitmaps_left: count, cell_density, cluster_weight }
}

/// One bitmap, for a caller that wants a single bitmap rather than a
/// run of them.
pub fn one_grown(seed: u64, cell_density: f64, cluster_weight: f64) -> Bitmap {
    generate::grow_one_bitmap(seed, cell_density, cluster_weight)
}

impl Iterator for GrownBitmaps {
    type Item = Bitmap;

    fn next(&mut self) -> Option<Bitmap> {
        if self.bitmaps_left == 0 {
            return None;
        }
        self.bitmaps_left -= 1;
        let seed = self.seed;
        self.seed += 1;
        Some(generate::grow_one_bitmap(seed, self.cell_density, self.cluster_weight))
    }
}

impl ExactSizeIterator for GrownBitmaps {
    fn len(&self) -> usize {
        self.bitmaps_left as usize
    }
}

/// How many bitmaps a family takes from each of its generators: a
/// shape, a plan or a line set.
#[derive(Clone, Copy, Debug)]
pub enum HowMany {
    /// As many as a timed run of that generator should take: its own
    /// count.
    TimedCount,
    /// As many as a test of that generator should take: the fast
    /// tier's own, few.
    TestedCount,
    /// The same number of each.
    SameCountEach(u64),
}

impl HowMany {
    /// How many to take of a generator whose own counts are `timed_count` and
    /// `tested_count`.
    pub fn to_take(self, timed_count: u64, tested_count: u64) -> u64 {
        match self {
            HowMany::TimedCount => timed_count,
            HowMany::TestedCount => tested_count,
            HowMany::SameCountEach(count) => count,
        }
    }
}

/// Bitmaps each generator makes for a timing, unless told otherwise:
/// enough, over every generator, for a steady mean and a tail.
pub const TIMING_PER_GENERATOR: u64 = 100;

/// Every family of corpus bitmaps grown from `seed`, named, with `how_many` of each
/// generator's: a result on one family is a quarter of a result
/// (`docs/lab.md`, "`corpus/`").
pub fn corpus_families(seed: u64, how_many: HowMany) -> Vec<(String, Vec<Bitmap>)> {
    vec![
        ("laid out like a city".to_string(), CITY_PLANS.iter().flat_map(|city_plan| city_plan.take_bitmaps(seed, how_many.to_take(city_plan.timed_count, city_plan.tested_count))).collect()),
        ("grown like a blob".to_string(), GROWN_SHAPES.iter().flat_map(|grown_shape| grown_shape.take_bitmaps(seed, how_many.to_take(grown_shape.timed_count, grown_shape.tested_count))).collect()),
        ("sparse".to_string(), SPARSE_SHAPES.iter().flat_map(|grown_shape| grown_shape.take_bitmaps(seed, how_many.to_take(grown_shape.timed_count, grown_shape.tested_count))).collect()),
        ("drawn with lines".to_string(), LINE_SETS.iter().flat_map(|line_set| line_set.take_bitmaps(seed, how_many.to_take(line_set.timed_count, line_set.tested_count))).collect()),
    ]
}
