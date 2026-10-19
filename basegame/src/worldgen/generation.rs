//! How a world is generated, as a whole: the heights' shape and how
//! grass and trees lie on them ([`WorldGeneration`]), saved with a world;
//! what grows on a cell; a seed with land where a flock is to stand
//! (`docs/worldgen.md`, "Generation").

use crate::worldgen::mesh::WorldLand;
use crate::worldgen::patches::Patches;
use crate::worldgen::{SmoothNoise, LandShape, FRACTION_ONE};
use server::simulation::simulation_api::{CellCartesian, SUPERCHUNK_SIDE_CELLS};
use crate::worldgen::lots::mix_word;

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

/// Where a generation keeps one of its worldgen parameters.
type NumberField = fn(&mut WorldGeneration) -> &mut dyn GenerationNumber;

/// Each of a generation's parameters, by its name: in the order a world's
/// file keeps them.
const PARAMETER_FIELDS: [(&str, NumberField); 28] = [
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
    /// Its worldgen parameters, each with its name: as a world's file
    /// keeps them.
    pub fn worldgen_parameters(&self) -> Vec<(String, u64)> {
        let mut generation = *self;
        PARAMETER_FIELDS.iter().map(|(name, field)| (name.to_string(), field(&mut generation).read())).collect()
    }

    /// As `worldgen_parameters` say, each by its name: one not among them
    /// as in [`WorldGeneration::DEFAULT`], one not known passed over.
    pub fn of_worldgen_parameters(worldgen_parameters: &[(String, u64)]) -> Self {
        let mut generation = Self::DEFAULT;
        for (name, value) in worldgen_parameters {
            if let Some((_, field)) = PARAMETER_FIELDS.iter().find(|(field_name, _)| field_name == name) {
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
