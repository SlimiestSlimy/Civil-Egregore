//! How a world is generated, as a whole: the heights' shape and how
//! grass and trees lie on them ([`WorldGeneration`]), saved with a world;
//! what grows on a cell; a seed with land where a flock is to stand
//! (`docs/worldgen.md`, "Generation").

use crate::worldgen::mesh::{WorldLand, SIGMOID_ONE};
use crate::worldgen::patches::Patches;
use crate::worldgen::{SmoothNoise, LandShape, FRACTION_ONE};
use server::simulation::simulation_api::{CellCartesian, SUPERCHUNK_SIDE_CELLS};
use crate::worldgen::lots::mix_word;
use server::simulation::simulation_api::tuning::{self, Tuning};

/// What keeps the trees' numbers apart from the grass's.
pub const TREES_SALT: u64 = 0x7472_6565_735F_6C6F;

/// Sheep a superchunk is given unless told otherwise.
pub const FLOCK: u64 = 4_000;

/// How superchunks are generated: the heights' shape, how the grass and
/// the trees lie on them, and the sheep each is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldGeneration {
    /// The heights' shape.
    pub land_shape: LandShape,
    /// How the grass lies.
    pub grass_patches: Patches,
    /// How the trees lie.
    pub trees: Patches,
    /// Sheep each superchunk is given the first time it is hot.
    pub sheep_per_superchunk: u64,
}

/// A number of a generation, kept in a field of whichever width: read
/// out as a world's file keeps it, and put back.
trait GenerationNumber {
    /// The number, widened.
    fn read(&self) -> u64;
    /// Puts `value` in, cut to the field's width.
    fn write(&mut self, value: u64);
}

impl GenerationNumber for u16 {
    fn read(&self) -> u64 {
        u64::from(*self)
    }

    fn write(&mut self, value: u64) {
        *self = value as u16;
    }
}

impl GenerationNumber for u32 {
    fn read(&self) -> u64 {
        u64::from(*self)
    }

    fn write(&mut self, value: u64) {
        *self = value as u32;
    }
}

impl GenerationNumber for u64 {
    fn read(&self) -> u64 {
        *self
    }

    fn write(&mut self, value: u64) {
        *self = value;
    }
}

/// Where a generation keeps one of its numbers.
type NumberField = fn(&mut WorldGeneration) -> &mut dyn GenerationNumber;

/// Each of a generation's numbers, by its name: in the order a world's
/// file keeps them.
const NUMBER_FIELDS: [(&str, NumberField); 28] = [
    ("ocean floor", |generation| &mut generation.land_shape.ocean_floor),
    ("ocean level", |generation| &mut generation.land_shape.ocean),
    ("vertex spacing", |generation| &mut generation.land_shape.vertex_spacing_power),
    ("ocean share", |generation| &mut generation.land_shape.ocean_share),
    ("highest land", |generation| &mut generation.land_shape.highest_land),
    ("clumping", |generation| &mut generation.land_shape.land_clumping),
    ("coast breadth", |generation| &mut generation.land_shape.coast_breadth),
    ("coast lowness", |generation| &mut generation.land_shape.coast_low),
    ("narrowest blend", |generation| &mut generation.land_shape.narrowest_blend),
    ("widest blend", |generation| &mut generation.land_shape.widest_blend),
    ("least sigmoid", |generation| &mut generation.land_shape.least_sigmoidness),
    ("most sigmoid", |generation| &mut generation.land_shape.most_sigmoidness),
    ("line bending", |generation| &mut generation.land_shape.line_bending),
    ("finer mesh depth", |generation| &mut generation.land_shape.finer_depth),
    ("finer mesh share", |generation| &mut generation.land_shape.finer_share),
    ("finer mesh height", |generation| &mut generation.land_shape.finer_height),
    ("finer mesh falloff", |generation| &mut generation.land_shape.finer_fall),
    ("weight spread", |generation| &mut generation.land_shape.weight),
    ("raised share", |generation| &mut generation.land_shape.raised_share),
    ("grass cover", |generation| &mut generation.grass_patches.cover_share),
    ("grass patch size", |generation| &mut generation.grass_patches.patch_size_power),
    ("grass patch detail", |generation| &mut generation.grass_patches.detail_share),
    ("grass scatter", |generation| &mut generation.grass_patches.scatter_share),
    ("tree cover", |generation| &mut generation.trees.cover_share),
    ("tree patch size", |generation| &mut generation.trees.patch_size_power),
    ("tree patch detail", |generation| &mut generation.trees.detail_share),
    ("tree scatter", |generation| &mut generation.trees.scatter_share),
    ("sheep per superchunk", |generation| &mut generation.sheep_per_superchunk),
];

impl WorldGeneration {
    /// Its numbers, each with its name: as a world's file keeps them.
    pub fn numbers(&self) -> Vec<(String, u64)> {
        let mut generation = *self;
        NUMBER_FIELDS.iter().map(|(name, field)| (name.to_string(), field(&mut generation).read())).collect()
    }

    /// As `generation_numbers` say, each by its name: one not among them
    /// as in [`WorldGeneration::DEFAULT`], one not known passed over.
    pub fn of_numbers(numbers: &[(String, u64)]) -> Self {
        let mut generation = Self::DEFAULT;
        for (name, value) in numbers {
            if let Some((_, field)) = NUMBER_FIELDS.iter().find(|(field_name, _)| field_name == name) {
                field(&mut generation).write(*value);
            }
        }
        generation
    }
}

impl WorldGeneration {
    /// A plain: dry land flat throughout, no ocean and no wall on it,
    /// no trees, and grass scattered cell by cell on `grass_cover` of
    /// [`ONE`] of its cells. What a rule is measured and tested on
    /// alone, the terrain taking no part.
    pub const fn plain(grass_cover: u64) -> Self {
        let plain_shape = LandShape { ocean_floor: 0, ocean: 0, highest_land: 1, ocean_share: 0, coast_breadth: 0, finer_depth: 0, ..LandShape::DEFAULT };
        Self { land_shape: plain_shape, grass_patches: Patches { cover_share: grass_cover, patch_size_power: 1, detail_share: 0, scatter_share: 16 * FRACTION_ONE }, trees: Patches { cover_share: 0, patch_size_power: 1, detail_share: 0, scatter_share: 0 }, sheep_per_superchunk: 0 }
    }

    /// How worlds are generated unless told otherwise, as tuned by eye.
    pub const DEFAULT: Self = Self {
        land_shape: LandShape::DEFAULT,
        grass_patches: Patches { cover_share: FRACTION_ONE * 951 / 1000, patch_size_power: 8, detail_share: FRACTION_ONE * 598 / 1000, scatter_share: FRACTION_ONE * 51 / 1000 },
        trees: Patches { cover_share: FRACTION_ONE * 60 / 1000, patch_size_power: 7, detail_share: FRACTION_ONE * 800 / 1000, scatter_share: FRACTION_ONE * 300 / 1000 },
        sheep_per_superchunk: FLOCK,
    };

    /// As the sliders' numbers have it: what is typed held to what a
    /// height, the mesh and the noise can take.
    pub fn from_tuning(slider_tuning: &Tuning) -> Self {
        let of_one = |share_typed: f32| (share_typed.clamp(0.0, 1024.0) * FRACTION_ONE as f32) as u64;
        let patches = |[cover_slider, patch_slider, detail_slider, scatter_slider]: [usize; 4]| Patches { cover_share: of_one(slider_tuning[cover_slider]), patch_size_power: slider_tuning[patch_slider].round().clamp(0.0, 16.0) as u32, detail_share: of_one(slider_tuning[detail_slider]), scatter_share: of_one(slider_tuning[scatter_slider]) };
        Self {
            land_shape: shape_from_tuning(slider_tuning),
            grass_patches: patches([tuning::GRASS_COVER, tuning::GRASS_PATCH, tuning::GRASS_DETAIL, tuning::GRASS_SCATTER]),
            trees: patches([tuning::TREE_COVER, tuning::TREE_PATCH, tuning::TREE_DETAIL, tuning::TREE_SCATTER]),
            sheep_per_superchunk: slider_tuning[tuning::SHEEP_PER_SUPERCHUNK].max(0.0) as u64,
        }
    }

    /// What grows in a world of `seed` generated so: the thresholds
    /// found once, each cell then asked ([`WorldGrowth::grown_at`]).
    pub fn world_growth(&self, seed: u64) -> WorldGrowth {
        let trees_seed = seed ^ TREES_SALT;
        WorldGrowth {
            grass_patches: self.grass_patches,
            trees: self.trees,
            seed,
            trees_seed,
            grass_noises: self.grass_patches.patch_noises(seed),
            trees_noises: self.trees.patch_noises(trees_seed),
            grass_under: self.grass_patches.cover_threshold(seed) * self.grass_patches.weights(),
            trees_under: self.trees.cover_threshold(trees_seed) * self.trees.weights(),
        }
    }
}

/// The heights' shape, as the sliders' numbers have it.
fn shape_from_tuning(slider_tuning: &Tuning) -> LandShape {
    let height = |slider_value: f32| slider_value.round().clamp(0.0, u16::MAX as f32) as u16;
    let to_share = |slider_value: f32| (slider_value.clamp(0.0, 1.0) * FRACTION_ONE as f32) as u64;
    let to_sigmoid = |slider_value: f32| (slider_value.clamp(1.0, 16.0) * SIGMOID_ONE as f32) as u64;
    let ocean_floor = height(slider_tuning[tuning::OCEAN_FLOOR]);
    LandShape {
        ocean_floor,
        // No ocean under its own floor.
        ocean: height(slider_tuning[tuning::OCEAN_LEVEL]).max(ocean_floor),
        vertex_spacing_power: slider_tuning[tuning::VERTEX_SPACING].round().clamp(6.0, 24.0) as u32,
        ocean_share: to_share(slider_tuning[tuning::OCEAN_SHARE]),
        highest_land: height(slider_tuning[tuning::HIGHEST_LAND]),
        coast_breadth: slider_tuning[tuning::COAST_BREADTH].round().clamp(0.0, 4.0) as u32,
        coast_low: to_sigmoid(slider_tuning[tuning::COAST_LOWNESS]),
        land_clumping: to_share(slider_tuning[tuning::LAND_CLUMPING]),
        narrowest_blend: to_share(slider_tuning[tuning::NARROWEST_BLEND]),
        widest_blend: to_share(slider_tuning[tuning::WIDEST_BLEND]),
        least_sigmoidness: to_sigmoid(slider_tuning[tuning::LEAST_SIGMOID]),
        most_sigmoidness: to_sigmoid(slider_tuning[tuning::MOST_SIGMOID]),
        line_bending: (slider_tuning[tuning::LINE_BENDING].clamp(0.0, 4.0) * FRACTION_ONE as f32) as u64,
        finer_depth: slider_tuning[tuning::FINER_DEPTH].round().clamp(0.0, 10.0) as u32,
        finer_fall: to_share(slider_tuning[tuning::FINER_FALL]),
        weight: to_share(slider_tuning[tuning::WEIGHT_SPREAD]),
        finer_share: to_share(slider_tuning[tuning::FINER_SHARE]),
        finer_height: height(slider_tuning[tuning::FINER_HEIGHT]) as u64,
        raised_share: to_share(slider_tuning[tuning::RAISED_SHARE]),
    }
}

/// What grows where in one world: how the grass and the trees lie, and
/// the thresholds under which a cell's numbers have them.
#[derive(Clone, Copy, Debug)]
pub struct WorldGrowth {
    /// How the grass lies.
    grass_patches: Patches,
    /// How the trees lie.
    trees: Patches,
    /// The world's seed: the grass's.
    seed: u64,
    /// The trees' seed, apart from the grass's.
    trees_seed: u64,
    /// The grass's noises, kept cell after cell.
    grass_noises: [SmoothNoise; 2],
    /// The trees'.
    trees_noises: [SmoothNoise; 2],
    /// A cell whose grass number, weighed ([`Patches::weighed_cartesian`]), is
    /// under this has grass: no division a cell.
    grass_under: u64,
    /// A cell whose trees number, weighed, is under this has a tree.
    trees_under: u64,
}

/// What a dry cell starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrownCell {
    /// Grass, or dirt.
    pub has_grass: bool,
    /// A tree: its lot, a stage drawn from it by whoever counts the
    /// stages.
    pub tree: Option<u64>,
}

impl WorldGrowth {
    /// What the dry cell at `(x, y)` starts with.
    pub fn grown_at(&mut self, x: u32, y: u32) -> GrownCell {
        let weigh_cell = |patches: &Patches, patch_noises: &mut [SmoothNoise; 2], seed: u64| patches.weighed_cartesian(seed, patch_noises[0].noise_at(x, y), patch_noises[1].noise_at(x, y), x, y);
        let has_grass = weigh_cell(&self.grass_patches, &mut self.grass_noises, self.seed) < self.grass_under;
        let tree = (weigh_cell(&self.trees, &mut self.trees_noises, self.trees_seed) < self.trees_under).then(|| mix_word(self.trees_seed ^ ((y as u64) << 32 | x as u64)));
        GrownCell { has_grass, tree }
    }
}

/// Whether the world of `seed`, shaped as `land_shape`, has land about
/// `middle_cell`: seven cells across and down, a superchunk's side apart.
pub fn has_land_about(seed: u64, land_shape: &LandShape, middle_cell: CellCartesian) -> bool {
    let side_cells = SUPERCHUNK_SIDE_CELLS as i32;
    let mut land_heights = WorldLand::new(land_shape, seed);
    (-3i32..=3).all(|superchunks_across| (-3i32..=3).all(|superchunks_down| land_heights.height(middle_cell.x.wrapping_add_signed(superchunks_across * side_cells), middle_cell.y.wrapping_add_signed(superchunks_down * side_cells)) > land_shape.ocean))
}

/// The first seed from `first_seed` on whose world, shaped as `land_shape`, has
/// land about `middle_cell` ([`has_land_about`]): what a world is made from to
/// be watched or tested with a flock on it, the seed otherwise as
/// likely to give ocean there.
pub fn seed_with_land(first_seed: u64, land_shape: &LandShape, middle_cell: CellCartesian) -> u64 {
    (first_seed..).find(|&seed| has_land_about(seed, land_shape, middle_cell)).expect("a seed with land about where it is to")
}
