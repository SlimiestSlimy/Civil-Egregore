//! What is tuned by eye: a few numbers the window's sliders
//! ([`crate::sliders`]) set -- the near view's shading, which the
//! painter reads each frame, and in the lab ([`crate::lab`]) how the
//! world is generated -- with no lock between them. They are kept from one run to the next
//! ([`path`]), so what was found by eye can be read back and written
//! into the code as the defaults.

use std::fs::{create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// One number to tune.
pub struct Tuned {
    /// Its name, as shown and as saved.
    pub name: &'static str,
    /// What it is unless set.
    pub default: f32,
    /// The least and the most a slider sets it to.
    pub range: (f32, f32),
    /// The page of sliders it is on.
    pub page: Page,
    /// What it does, said to whoever rests the pointer on its slider.
    pub what: &'static str,
}

/// A page of sliders: the numbers tuned together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The near view's shading.
    Shading,
    /// How the world is generated: its heights and its grass.
    Generation,
}

/// How much lighter a step's line towards the sun is.
pub const STEP_LIGHT: usize = 0;
/// How much darker a step's line away from the sun is.
pub const STEP_DARK: usize = 1;
/// How much darker the foot of a wall's band is, the wall facing away
/// from the sun.
pub const WALL_SHADE: usize = 2;
/// How much darker it is, the sun on the wall.
pub const WALL_LIT: usize = 3;
/// How much of that the band has lost at its far edge.
pub const WALL_FADE: usize = 4;
/// How much darker a cast shadow is.
pub const SHADOW: usize = 5;
/// How much of the slopes' light and the heights' tint is shown.
pub const RELIEF: usize = 6;
/// How much the ground's pixels differ by lot.
pub const TEXTURE: usize = 7;

/// The height of the ocean's floor: the lowest ground there is.
pub const OCEAN_FLOOR: usize = 8;
/// The height the ocean stands at.
pub const OCEAN_LEVEL: usize = 9;
/// The height the highest land polygon's plain may stand at.
pub const HIGHEST_PLAIN: usize = 10;
/// The share of the polygons that are ocean.
pub const OCEAN_SHARE: usize = 11;
/// The cells along a square of the polygons' grid, as a power of two.
pub const POLYGON_SIZE: usize = 12;
/// The cells the narrowest polygon's ramp is across.
pub const NARROWEST_RAMP: usize = 13;
/// The cells the widest polygon's ramp is across.
pub const WIDEST_RAMP: usize = 14;
/// How hard the softest polygon's ramp is.
pub const SOFTEST_RAMP: usize = 15;
/// How hard the hardest polygon's ramp is.
pub const HARDEST_RAMP: usize = 16;
/// How far the polygons' borders are bent, beside a square's side.
pub const BORDER_BENDING: usize = 17;
/// The grids of smaller polygons within the land ones.
pub const INNER_DEPTH: usize = 18;
/// The share of the smaller polygons that raise or sink the ground.
pub const INNER_SHARE: usize = 19;
/// The most one of them raises or sinks it.
pub const INNER_HEIGHT: usize = 20;
/// The share of those that raise it.
pub const RAISED_SHARE: usize = 21;
/// The lines a land polygon has at most.
pub const LINES: usize = 22;
/// The most a line raises or sinks the ground.
pub const LINE_HEIGHT: usize = 23;
/// The share of the lines that are ridges.
pub const RIDGE_SHARE: usize = 24;
/// The cells from it the narrowest line is gone at.
pub const NARROWEST_LINE: usize = 25;
/// The cells from it the widest line is gone at.
pub const WIDEST_LINE: usize = 26;
/// The share of the cells that are grass.
pub const GRASS_COVER: usize = 27;
/// The cells across a patch of grass, as a power of two.
pub const GRASS_PATCH: usize = 28;
/// How much finer noise counts beside the patches'.
pub const GRASS_DETAIL: usize = 29;
/// How much each cell's own lot counts: grass scattered, not in patches.
pub const GRASS_SCATTER: usize = 30;
/// The share of the cells that have a tree.
pub const TREE_COVER: usize = 31;
/// The cells across a patch of trees, as a power of two.
pub const TREE_PATCH: usize = 32;
/// How much finer noise counts beside the trees' patches'.
pub const TREE_DETAIL: usize = 33;
/// How much each cell's own lot counts for trees.
pub const TREE_SCATTER: usize = 34;

/// The numbers, in the order above; the shading's defaults are what was
/// found by eye with the sliders.
pub const TUNED: [Tuned; 35] = [
    Tuned { name: "step light", default: 0.35, range: (0.0, 1.0), page: Page::Shading, what: "How much lighter the border of a higher cell is where it faces the sun." },
    Tuned { name: "step dark", default: 0.35, range: (0.0, 0.8), page: Page::Shading, what: "How much darker the border of a higher cell is where it faces away from the sun." },
    Tuned { name: "wall shade", default: 0.49, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side away from the sun." },
    Tuned { name: "wall lit", default: 0.55, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side the sun is on." },
    Tuned { name: "wall fade", default: 0.8, range: (0.0, 1.5), page: Page::Shading, what: "How much of its darkness a wall's band has lost at its far edge." },
    Tuned { name: "shadow", default: 0.4, range: (0.0, 0.8), page: Page::Shading, what: "How much darker ground is under a cast shadow." },
    Tuned { name: "relief", default: 0.7, range: (0.0, 3.0), page: Page::Shading, what: "How strongly slopes are lit and heights tinted." },
    Tuned { name: "texture", default: 2.0, range: (0.0, 4.0), page: Page::Shading, what: "How much the ground's pixels differ from one another by lot." },
    Tuned { name: "ocean floor level", default: 256.0, range: (0.0, 4096.0), page: Page::Generation, what: "The height of the ocean's floor, and the lowest ground in the world. Every ocean polygon is flat at this height; shores climb from it." },
    Tuned { name: "ocean level", default: 511.0, range: (0.0, 8192.0), page: Page::Generation, what: "The height of the ocean's surface, the same all over the world. Ground under it is under water. Set under the floor, it is held to the floor: no ocean. Water keeps a depth to 255, so a floor more than 255 under this is drawn and kept as 255 deep." },
    Tuned { name: "highest plain", default: 711.0, range: (0.0, 8192.0), page: Page::Generation, what: "The height of the highest land there can be. Each land polygon is a flat plain at a height of its own, drawn by lot, between just over the ocean level and this. At or under the ocean level, all land lies just over the ocean." },
    Tuned { name: "ocean share", default: 0.5, range: (0.0, 1.0), page: Page::Generation, what: "The share of the polygons that are ocean; the rest are land. 0 is all land, 1 all ocean. Land polygons that touch make one island." },
    Tuned { name: "polygon size (2^)", default: 13.0, range: (6.0, 18.0), page: Page::Generation, what: "How broad one polygon is: 2 to this power, in cells. 10 is one superchunk, 13 is 8 superchunks, 16 is 64." },
    Tuned { name: "narrowest ramp", default: 4.0, range: (1.0, 4096.0), page: Page::Generation, what: "Every polygon has a ramp width of its own, drawn by lot between this and the widest ramp: how many cells its border's change of height is spread over. A polygon with a narrow ramp is ringed by cliffs -- a mesa if it stands high, a walled basin if low. Set both the same for one width everywhere." },
    Tuned { name: "widest ramp", default: 2048.0, range: (1.0, 4096.0), page: Page::Generation, what: "The broadest ramp a polygon can draw: its border's change of height spread over this many cells, a gentle slope or a shallow shore. Half a polygon's breadth at most." },
    Tuned { name: "softest ramp", default: 1.0, range: (1.0, 16.0), page: Page::Generation, what: "Every polygon also has a hardness of its own, drawn by lot between this and the hardest ramp: the shape of the change of height across its ramp. 1 is an even slope from one level to the other. Higher is more of a sigmoid: the two levels stay flat nearly to the border and the change is a step in the middle -- a cliff, even where the ramp is wide." },
    Tuned { name: "hardest ramp", default: 8.0, range: (1.0, 16.0), page: Page::Generation, what: "The hardest a polygon's ramp can draw. Set both the same for one shape of ramp everywhere: 1 and 1 for slopes only, 16 and 16 for cliffs only." },
    Tuned { name: "border bending", default: 0.3, range: (0.0, 1.0), page: Page::Generation, what: "How far borders are pushed out of line by noise, as a share of a polygon's breadth. 0 leaves straight-sided polygons; more makes bays, headlands and winding borders." },
    Tuned { name: "inner polygon depth", default: 2.0, range: (0.0, 3.0), page: Page::Generation, what: "How many grids of smaller polygons lie inside the land polygons: each grid's polygons a quarter as broad as the one before, and each changing the ground by half as much: a big polygon cut into smaller ones, a small variation at a time. 0 for none: every land polygon one flat plain." },
    Tuned { name: "inner polygon share", default: 0.7, range: (0.0, 1.0), page: Page::Generation, what: "The share of the smaller polygons that raise or sink the ground; the rest leave their polygon's plain as it is." },
    Tuned { name: "inner polygon height", default: 120.0, range: (0.0, 2048.0), page: Page::Generation, what: "How many heights a smaller polygon raises or sinks the ground at most: each by an amount of its own, drawn by lot, and half as much for each grid finer." },
    Tuned { name: "raised share", default: 0.6, range: (0.0, 1.0), page: Page::Generation, what: "The share of those smaller polygons that raise the ground -- plateaus, mesas; the rest sink it -- basins. One sunk under the ocean level fills with water." },
    Tuned { name: "lines per polygon", default: 2.0, range: (0.0, 4.0), page: Page::Generation, what: "How many lines a land polygon has at most: each has from none to this many, by lot. A line is a chain of three segments about the polygon's middle: a ridge or a canyon." },
    Tuned { name: "line height", default: 300.0, range: (0.0, 4096.0), page: Page::Generation, what: "How many heights a line raises the ground along it -- a ridge -- or sinks it -- a canyon -- at most: each by an amount of its own, drawn by lot." },
    Tuned { name: "ridge share", default: 0.7, range: (0.0, 1.0), page: Page::Generation, what: "The share of the lines that are ridges; the rest are canyons. A canyon cut under the ocean level fills with water." },
    Tuned { name: "narrowest line", default: 64.0, range: (1.0, 4096.0), page: Page::Generation, what: "Every line has a width of its own, drawn by lot between this and the widest line: how many cells from the line its rise or cut is gone. A narrow line is a wall-sided ridge or a slot canyon." },
    Tuned { name: "widest line", default: 1024.0, range: (1.0, 4096.0), page: Page::Generation, what: "The broadest a line can draw: a mountain range with long flanks, or a wide valley." },
    Tuned { name: "grass cover", default: 0.95, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts as grass; the rest is dirt." },
    Tuned { name: "grass patch size (2^)", default: 8.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a patch of grass or of dirt is: 2 to this power, in cells." },
    Tuned { name: "grass patch detail", default: 0.6, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the patches' edges are: how much finer noise counts beside the patches'." },
    Tuned { name: "grass scatter", default: 0.05, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: grass scattered cell by cell, not in patches." },
    Tuned { name: "tree cover", default: 0.06, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts with a tree." },
    Tuned { name: "tree patch size (2^)", default: 7.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a wood is: 2 to this power, in cells." },
    Tuned { name: "tree patch detail", default: 0.8, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the woods' edges are." },
    Tuned { name: "tree scatter", default: 0.3, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: lone trees, not woods." },
];

/// Counts the changes to how the world is generated: what was made
/// under an earlier count is made again.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How many times how the world is generated has changed.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Says that how the world is generated has changed.
pub fn regenerate() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The numbers as they are now, each a float's bits.
static VALUES: [AtomicU32; TUNED.len()] = [const { AtomicU32::new(0) }; TUNED.len()];

/// The numbers, read together.
pub type Tuning = [f32; TUNED.len()];

/// Where the numbers are kept between runs: a line each, its name and
/// its value. Under the crate's folder, out of git.
pub fn path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("transient_data/tuning.txt")
}

/// Sets every number to its default, then to what was kept, if any.
pub fn start() {
    let kept = read_to_string(path()).unwrap_or_default();
    for (index, tuned) in TUNED.iter().enumerate() {
        let kept = kept.lines().find_map(|line| line.strip_prefix(tuned.name)?.strip_prefix(" = ")?.trim().parse().ok());
        set(index, kept.unwrap_or(tuned.default));
    }
}

/// The numbers now.
pub fn now() -> Tuning {
    std::array::from_fn(|index| f32::from_bits(VALUES[index].load(Ordering::Relaxed)))
}

/// Sets the `index`-th number -- to anything: its range is only what
/// its slider reaches, and a value typed may pass it. The world is to
/// be generated again if it is one of generation's and has changed.
pub fn set(index: usize, value: f32) {
    let bits = if value.is_finite() { value } else { TUNED[index].default }.to_bits();
    if VALUES[index].swap(bits, Ordering::Relaxed) != bits && TUNED[index].page == Page::Generation {
        regenerate();
    }
}

/// Keeps the numbers for the next run. A failure is let pass: they are
/// then only not kept.
pub fn keep() {
    let lines: String = TUNED.iter().zip(now()).map(|(tuned, value)| format!("{} = {value:.3}\n", tuned.name)).collect();
    let path = path();
    _ = path.parent().map(create_dir_all);
    _ = write(path, lines);
}
