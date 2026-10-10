//! How a world is generated, as a whole: the heights' shape and how
//! grass and trees lie on them ([`Generation`]), saved with a world;
//! what grows on a cell; a seed with land where a flock is to stand
//! (`docs/worldgen.md`, "Generation").

use crate::mesh::{Lands, SIGMOID_ONE};
use crate::patches::Patches;
use crate::{Shape, ONE};
use coordinates::{SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use utilities::hash::mix;
use utilities::tuning::{self, Tuning};

/// What keeps the trees' numbers apart from the grass's.
pub const TREES_SALT: u64 = 0x7472_6565_735F_6C6F;

/// How superchunks are generated: the heights' shape, and how the
/// grass and the trees lie on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation {
    /// The heights' shape.
    pub shape: Shape,
    /// How the grass lies.
    pub grass: Patches,
    /// How the trees lie.
    pub trees: Patches,
}

/// Gives [`Generation`] its numbers by name: read out, and put back.
macro_rules! numbers {
    ($($name:literal => $($field:ident).+,)*) => {
        impl Generation {
            /// Its numbers, each with its name: as a world's file keeps them.
            pub fn numbers(&self) -> Vec<(String, u64)> {
                vec![$(($name.to_string(), u64::from(self.$($field).+))),*]
            }

            /// As `numbers` say, each by its name: one not among them
            /// as in [`Generation::DEFAULT`], one not known passed over.
            pub fn of_numbers(numbers: &[(String, u64)]) -> Self {
                let mut generation = Self::DEFAULT;
                for (name, value) in numbers {
                    match name.as_str() {
                        $($name => generation.$($field).+ = *value as _,)*
                        _ => {}
                    }
                }
                generation
            }
        }
    };
}

numbers! {
    "ocean floor" => shape.ground,
    "ocean level" => shape.ocean,
    "vertex spacing" => shape.span,
    "ocean share" => shape.sea,
    "highest land" => shape.highest,
    "clumping" => shape.clumping,
    "coast breadth" => shape.coast,
    "coast lowness" => shape.coast_low,
    "narrowest blend" => shape.narrow,
    "widest blend" => shape.wide,
    "least sigmoid" => shape.soft,
    "most sigmoid" => shape.hard,
    "line bending" => shape.warp,
    "finer mesh depth" => shape.finer_depth,
    "finer mesh share" => shape.finer_share,
    "finer mesh height" => shape.finer_height,
    "finer mesh falloff" => shape.finer_fall,
    "weight spread" => shape.weight,
    "raised share" => shape.raised,
    "grass cover" => grass.cover,
    "grass patch size" => grass.patch,
    "grass patch detail" => grass.detail,
    "grass scatter" => grass.scatter,
    "tree cover" => trees.cover,
    "tree patch size" => trees.patch,
    "tree patch detail" => trees.detail,
    "tree scatter" => trees.scatter,
}

impl Generation {
    /// A plain: dry land flat throughout, no ocean and no wall on it,
    /// no trees, and grass scattered cell by cell on `grass_cover` of
    /// [`ONE`] of its cells. What a rule is measured and tested on
    /// alone, the terrain taking no part.
    pub const fn plain(grass_cover: u64) -> Self {
        let shape = Shape { ground: 0, ocean: 0, highest: 1, sea: 0, coast: 0, finer_depth: 0, ..Shape::DEFAULT };
        Self { shape, grass: Patches { cover: grass_cover, patch: 1, detail: 0, scatter: 16 * ONE }, trees: Patches { cover: 0, patch: 1, detail: 0, scatter: 0 } }
    }

    /// How worlds are generated unless told otherwise, as tuned by eye.
    pub const DEFAULT: Self = Self {
        shape: Shape::DEFAULT,
        grass: Patches { cover: ONE * 951 / 1000, patch: 8, detail: ONE * 598 / 1000, scatter: ONE * 51 / 1000 },
        trees: Patches { cover: ONE * 60 / 1000, patch: 7, detail: ONE * 800 / 1000, scatter: ONE * 300 / 1000 },
    };

    /// As the sliders' numbers have it: what is typed held to what a
    /// height, the mesh and the noise can take.
    pub fn from_tuning(tuned: &Tuning) -> Self {
        let of_one = |share: f32| (share.clamp(0.0, 1024.0) * ONE as f32) as u64;
        let patches = |[cover, patch, detail, scatter]: [usize; 4]| Patches { cover: of_one(tuned[cover]), patch: tuned[patch].round().clamp(0.0, 16.0) as u32, detail: of_one(tuned[detail]), scatter: of_one(tuned[scatter]) };
        Self {
            shape: shape_from_tuning(tuned),
            grass: patches([tuning::GRASS_COVER, tuning::GRASS_PATCH, tuning::GRASS_DETAIL, tuning::GRASS_SCATTER]),
            trees: patches([tuning::TREE_COVER, tuning::TREE_PATCH, tuning::TREE_DETAIL, tuning::TREE_SCATTER]),
        }
    }

    /// What grows in a world of `seed` generated so: the thresholds
    /// found once, each cell then asked ([`Growth::at`]).
    pub fn growth(&self, seed: u64) -> Growth {
        let trees_seed = seed ^ TREES_SALT;
        Growth { grass: self.grass, trees: self.trees, seed, trees_seed, grass_under: self.grass.threshold(seed), trees_under: self.trees.threshold(trees_seed) }
    }
}

/// The heights' shape, as the sliders' numbers have it.
fn shape_from_tuning(tuned: &Tuning) -> Shape {
    let height = |tuned: f32| tuned.round().clamp(0.0, u16::MAX as f32) as u16;
    let share = |tuned: f32| (tuned.clamp(0.0, 1.0) * ONE as f32) as u64;
    let sigmoid = |tuned: f32| (tuned.clamp(1.0, 16.0) * SIGMOID_ONE as f32) as u64;
    let ground = height(tuned[tuning::OCEAN_FLOOR]);
    Shape {
        ground,
        // No ocean under its own floor.
        ocean: height(tuned[tuning::OCEAN_LEVEL]).max(ground),
        span: tuned[tuning::VERTEX_SPACING].round().clamp(6.0, 24.0) as u32,
        sea: share(tuned[tuning::OCEAN_SHARE]),
        highest: height(tuned[tuning::HIGHEST_LAND]),
        coast: tuned[tuning::COAST_BREADTH].round().clamp(0.0, 4.0) as u32,
        coast_low: sigmoid(tuned[tuning::COAST_LOWNESS]),
        clumping: share(tuned[tuning::CLUMPING]),
        narrow: share(tuned[tuning::NARROWEST_BLEND]),
        wide: share(tuned[tuning::WIDEST_BLEND]),
        soft: sigmoid(tuned[tuning::LEAST_SIGMOID]),
        hard: sigmoid(tuned[tuning::MOST_SIGMOID]),
        warp: (tuned[tuning::LINE_BENDING].clamp(0.0, 4.0) * ONE as f32) as u64,
        finer_depth: tuned[tuning::FINER_DEPTH].round().clamp(0.0, 10.0) as u32,
        finer_fall: share(tuned[tuning::FINER_FALL]),
        weight: share(tuned[tuning::WEIGHT_SPREAD]),
        finer_share: share(tuned[tuning::FINER_SHARE]),
        finer_height: height(tuned[tuning::FINER_HEIGHT]) as u64,
        raised: share(tuned[tuning::RAISED_SHARE]),
    }
}

/// What grows where in one world: how the grass and the trees lie, and
/// the thresholds under which a cell's numbers have them.
#[derive(Clone, Copy, Debug)]
pub struct Growth {
    /// How the grass lies.
    grass: Patches,
    /// How the trees lie.
    trees: Patches,
    /// The world's seed: the grass's.
    seed: u64,
    /// The trees' seed, apart from the grass's.
    trees_seed: u64,
    /// A cell whose grass number is under this has grass.
    grass_under: u64,
    /// A cell whose trees number is under this has a tree.
    trees_under: u64,
}

/// What a dry cell starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grown {
    /// Grass, or dirt.
    pub grass: bool,
    /// A tree: its lot, a stage drawn from it by whoever counts the
    /// stages.
    pub tree: Option<u64>,
}

impl Growth {
    /// What the dry cell at `(x, y)` starts with.
    pub fn at(&self, x: u32, y: u32) -> Grown {
        let grass = self.grass.number(self.seed, x, y) < self.grass_under;
        let tree = (self.trees.number(self.trees_seed, x, y) < self.trees_under).then(|| mix(self.trees_seed ^ ((y as u64) << 32 | x as u64)));
        Grown { grass, tree }
    }
}

/// Whether the world of `seed`, shaped as `shape`, has land about
/// `near`: three superchunks each way.
pub fn has_land_about(seed: u64, shape: &Shape, near: SuperchunkIndex) -> bool {
    let (middle, side) = (near.top_left().cartesian(), SUPERCHUNK_SIDE_CELLS as i32);
    let mut lands = Lands::new(shape, seed);
    (-3i32..=3).all(|across| (-3i32..=3).all(|down| lands.height(middle.x.wrapping_add_signed(across * side), middle.y.wrapping_add_signed(down * side)) > shape.ocean))
}

/// The first seed from `from` on whose world, shaped as `shape`, has
/// land about `near` ([`has_land_about`]): what a world is made from to
/// be watched or tested with a flock on it, the seed otherwise as
/// likely to give ocean there.
pub fn seed_with_land(from: u64, shape: &Shape, near: SuperchunkIndex) -> u64 {
    (from..).find(|&seed| has_land_about(seed, shape, near)).expect("a seed with land about where it is to")
}
