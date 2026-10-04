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

/// How much of a height the broadest octave makes up, 512 cells
/// across, beside the others.
pub const HILLS: usize = 8;
/// ...the next, 128 cells across.
pub const RIDGES: usize = 9;
/// ...the next, 32 cells across.
pub const BUMPS: usize = 10;
/// ...the finest, 8 cells across.
pub const ROUGHNESS: usize = 11;
/// The highest a cell can be: what the octaves make up together.
pub const HEIGHT_SPAN: usize = 12;
/// The share of the cells that are grass.
pub const GRASS_COVER: usize = 13;
/// The cells across a patch of grass, as a power of two.
pub const PATCH_SIZE: usize = 14;
/// How much finer noise counts beside the patches'.
pub const PATCH_DETAIL: usize = 15;
/// How much each cell's own lot counts: grass scattered, not in patches.
pub const SCATTER: usize = 16;
/// The share of the cells that have a tree.
pub const TREE_COVER: usize = 17;
/// The cells across a patch of trees, as a power of two.
pub const TREE_PATCH: usize = 18;
/// How much finer noise counts beside the trees' patches'.
pub const TREE_DETAIL: usize = 19;
/// How much each cell's own lot counts for trees.
pub const TREE_SCATTER: usize = 20;
/// The height the ocean stands at.
pub const OCEAN_LEVEL: usize = 21;
/// How far over the ocean the land is where the hills are whole.
pub const COAST: usize = 22;
/// The height the lowest ground is at.
pub const GROUND_LEVEL: usize = 23;
/// The most the land rises over the lowest ground.
pub const LAND_RISE: usize = 24;
/// The cells between two points of the land's rise, as a power of two.
pub const LAND_SPAN: usize = 25;

/// The numbers, in the order above; the shading's defaults are what was
/// found by eye with the sliders.
pub const TUNED: [Tuned; 26] = [
    Tuned { name: "step light", default: 0.35, range: (0.0, 1.0), page: Page::Shading, what: "How much lighter the border of a higher cell is where it faces the sun." },
    Tuned { name: "step dark", default: 0.35, range: (0.0, 0.8), page: Page::Shading, what: "How much darker the border of a higher cell is where it faces away from the sun." },
    Tuned { name: "wall shade", default: 0.49, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side away from the sun." },
    Tuned { name: "wall lit", default: 0.55, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side the sun is on." },
    Tuned { name: "wall fade", default: 0.8, range: (0.0, 1.5), page: Page::Shading, what: "How much of its darkness a wall's band has lost at its far edge." },
    Tuned { name: "shadow", default: 0.4, range: (0.0, 0.8), page: Page::Shading, what: "How much darker ground is under a cast shadow." },
    Tuned { name: "relief", default: 0.7, range: (0.0, 3.0), page: Page::Shading, what: "How strongly slopes are lit and heights tinted." },
    Tuned { name: "texture", default: 2.0, range: (0.0, 4.0), page: Page::Shading, what: "How much the ground's pixels differ from one another by lot." },
    Tuned { name: "hills (512)", default: 255.0, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the broadest octave, 512 cells across. Only its size beside the other three counts." },
    Tuned { name: "ridges (128)", default: 33.64, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 128 cells across." },
    Tuned { name: "bumps (32)", default: 91.85, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 32 cells across." },
    Tuned { name: "roughness (8)", default: 6.0, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the finest octave, 8 cells across: rough ground, many walls." },
    Tuned { name: "height span", default: 255.0, range: (0.0, 255.0), page: Page::Generation, what: "How high the hills stand over the land, in heights: 255 at most." },
    Tuned { name: "grass cover", default: 0.95, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts as grass." },
    Tuned { name: "patch size (2^)", default: 8.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a patch of grass or of dirt is: 2 to this power, in cells." },
    Tuned { name: "patch detail", default: 0.6, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the patches' edges are: how much finer noise counts beside the patches'." },
    Tuned { name: "scatter", default: 0.05, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: grass scattered cell by cell, not in patches." },
    Tuned { name: "tree cover", default: 0.06, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts with a tree." },
    Tuned { name: "tree patch (2^)", default: 7.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a wood is: 2 to this power, in cells." },
    Tuned { name: "tree detail", default: 0.8, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the woods' edges are." },
    Tuned { name: "tree scatter", default: 0.3, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: lone trees, not woods." },
    Tuned { name: "ocean level", default: 800.0, range: (0.0, 8192.0), page: Page::Generation, what: "The height the ocean stands at, all over the world. Land under it is ocean floor; land over it, islands." },
    Tuned { name: "coast", default: 64.0, range: (1.0, 1024.0), page: Page::Generation, what: "How many heights over the ocean the land is where the hills reach their whole height: from the shore to there they grow." },
    Tuned { name: "ground level", default: 256.0, range: (0.0, 4096.0), page: Page::Generation, what: "The height of the lowest ground there is: everything stands on it." },
    Tuned { name: "land rise", default: 1024.0, range: (0.0, 8192.0), page: Page::Generation, what: "How many heights the land rises over the lowest ground at most: the difference between the deepest ocean floor and the highest inland." },
    Tuned { name: "land span", default: 14.0, range: (8.0, 20.0), page: Page::Generation, what: "How broad the land's rises are: 2 to this power in cells between the noise's points. 14 is 16 superchunks." },
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
