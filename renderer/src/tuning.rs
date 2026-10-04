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

/// The first of the hills' eleven octaves, 8192 cells across to 8: how
/// much of the hills' height each makes up beside the others.
pub const HILLS: usize = 8;
/// The highest the hills stand: what their octaves make up together.
pub const HEIGHT_SPAN: usize = 19;
/// The share of the hills' height that lies under the land.
pub const HILL_SINK: usize = 20;
/// The height the lowest ground is at.
pub const GROUND_LEVEL: usize = 21;
/// The most the land rises over the lowest ground.
pub const LAND_RISE: usize = 22;
/// The cells between two points of the land's rise, as a power of two.
pub const LAND_SPAN: usize = 23;
/// The first of the five octaves of the land's rise: each one's share.
pub const RISE_SHARES: usize = 24;
/// The share of the world that is under the ocean.
pub const OCEAN_SHARE: usize = 29;
/// How far under the ocean its deepest floor is.
pub const OCEAN_DEPTH: usize = 30;
/// How many heights of land the hills grow to their whole height over.
pub const COAST: usize = 31;
/// How far where the hills begin wanders about the shore, in coasts.
pub const SHORE_WANDER: usize = 32;
/// The cells between two points of that wandering, as a power of two.
pub const SHORE_SPAN: usize = 33;
/// The share of the cells that are grass.
pub const GRASS_COVER: usize = 34;
/// The cells across a patch of grass, as a power of two.
pub const PATCH_SIZE: usize = 35;
/// How much finer noise counts beside the patches'.
pub const PATCH_DETAIL: usize = 36;
/// How much each cell's own lot counts: grass scattered, not in patches.
pub const SCATTER: usize = 37;
/// The share of the cells that have a tree.
pub const TREE_COVER: usize = 38;
/// The cells across a patch of trees, as a power of two.
pub const TREE_PATCH: usize = 39;
/// How much finer noise counts beside the trees' patches'.
pub const TREE_DETAIL: usize = 40;
/// How much each cell's own lot counts for trees.
pub const TREE_SCATTER: usize = 41;

/// The numbers, in the order above; the shading's defaults are what was
/// found by eye with the sliders.
pub const TUNED: [Tuned; 42] = [
    Tuned { name: "step light", default: 0.35, range: (0.0, 1.0), page: Page::Shading, what: "How much lighter the border of a higher cell is where it faces the sun." },
    Tuned { name: "step dark", default: 0.35, range: (0.0, 0.8), page: Page::Shading, what: "How much darker the border of a higher cell is where it faces away from the sun." },
    Tuned { name: "wall shade", default: 0.49, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side away from the sun." },
    Tuned { name: "wall lit", default: 0.55, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side the sun is on." },
    Tuned { name: "wall fade", default: 0.8, range: (0.0, 1.5), page: Page::Shading, what: "How much of its darkness a wall's band has lost at its far edge." },
    Tuned { name: "shadow", default: 0.4, range: (0.0, 0.8), page: Page::Shading, what: "How much darker ground is under a cast shadow." },
    Tuned { name: "relief", default: 0.7, range: (0.0, 3.0), page: Page::Shading, what: "How strongly slopes are lit and heights tinted." },
    Tuned { name: "texture", default: 2.0, range: (0.0, 4.0), page: Page::Shading, what: "How much the ground's pixels differ from one another by lot." },
    Tuned { name: "hills (8192)", default: 0.0, range: (0.0, 255.0), page: Page::Generation, what: "The height the hills' octave 8192 cells across adds at most, beside the others: the height span is shared out among them." },
    Tuned { name: "hills (4096)", default: 0.0, range: (0.0, 255.0), page: Page::Generation, what: "The height the hills' octave 4096 cells across adds at most, beside the others: the height span is shared out among them." },
    Tuned { name: "hills (2048)", default: 0.0, range: (0.0, 255.0), page: Page::Generation, what: "The height the hills' octave 2048 cells across adds at most, beside the others: the height span is shared out among them." },
    Tuned { name: "hills (1024)", default: 0.0, range: (0.0, 255.0), page: Page::Generation, what: "The height the hills' octave 1024 cells across adds at most, beside the others: the height span is shared out among them." },
    Tuned { name: "hills (512)", default: 255.0, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 512 cells across. Only its size beside the other six counts." },
    Tuned { name: "hills (256)", default: 144.32, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 256 cells across. Only its size beside the other six counts." },
    Tuned { name: "ridges (128)", default: 33.64, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 128 cells across. Only its size beside the other six counts." },
    Tuned { name: "ridges (64)", default: 62.75, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 64 cells across. Only its size beside the other six counts." },
    Tuned { name: "bumps (32)", default: 91.85, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 32 cells across. Only its size beside the other six counts." },
    Tuned { name: "bumps (16)", default: 48.93, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the octave 16 cells across. Only its size beside the other six counts." },
    Tuned { name: "roughness (8)", default: 6.0, range: (0.0, 255.0), page: Page::Generation, what: "The share of the hills' height from the finest octave, 8 cells across: rough ground, many walls." },
    Tuned { name: "height span", default: 255.0, range: (0.0, 4096.0), page: Page::Generation, what: "How high the hills stand over the land at most, in heights: what their octaves share out. Broad octaves take a great span without a wall; fine ones turn it all to walls." },
    Tuned { name: "hill sink", default: 0.0, range: (0.0, 1.0), page: Page::Generation, what: "The share of the hills' height span that lies under the land: 0, and hills only stand on the land; 0.5, and they are as much hollows as hills. A hollow that goes under the ocean's level where the land is over it stays dry." },
    Tuned { name: "ground level", default: 256.0, range: (0.0, 4096.0), page: Page::Generation, what: "The height of the lowest ground there is: everything stands on it." },
    Tuned { name: "land rise", default: 1024.0, range: (0.0, 8192.0), page: Page::Generation, what: "How many heights the land rises over the lowest ground at most: the difference between the deepest ocean floor and the highest inland." },
    Tuned { name: "land span", default: 14.0, range: (8.0, 20.0), page: Page::Generation, what: "How broad the land's rises are: 2 to this power in cells between the noise's points. 14 is 16 superchunks." },
    Tuned { name: "rise octave 1", default: 1.0, range: (0.0, 1.0), page: Page::Generation, what: "The share of the land's rise from its octave 1 of 5 -- the broadest, as broad as the land span says. Only its size beside the other four counts." },
    Tuned { name: "rise octave 2", default: 0.4, range: (0.0, 1.0), page: Page::Generation, what: "The share of the land's rise from its octave 2 of 5 -- half as broad as the first. Only its size beside the other four counts." },
    Tuned { name: "rise octave 3", default: 0.16, range: (0.0, 1.0), page: Page::Generation, what: "The share of the land's rise from its octave 3 of 5 -- a quarter as broad: bays and headlands. Only its size beside the other four counts." },
    Tuned { name: "rise octave 4", default: 0.064, range: (0.0, 1.0), page: Page::Generation, what: "The share of the land's rise from its octave 4 of 5 -- an eighth as broad: coves. Only its size beside the other four counts." },
    Tuned { name: "rise octave 5", default: 0.026, range: (0.0, 1.0), page: Page::Generation, what: "The share of the land's rise from its octave 5 of 5 -- a sixteenth as broad: a ragged shore, and steeper land. Only its size beside the other four counts." },
    Tuned { name: "ocean share", default: 0.55, range: (0.0, 1.0), page: Page::Generation, what: "The share of the world that is under the ocean: the ocean's level is set to whatever height that much of the land's rise is under, so it holds whatever the rise's sliders say. The level it comes to is shown with the seed." },
    Tuned { name: "ocean depth", default: 255.0, range: (0.0, 4096.0), page: Page::Generation, what: "How many heights under the ocean its deepest floor is: the land under the ocean falls gently to that, not all the way to the ground level. Water keeps a depth to 255; deeper is drawn and kept as 255." },
    Tuned { name: "coast", default: 64.0, range: (1.0, 1024.0), page: Page::Generation, what: "How many heights over the ocean the land is where the hills reach their whole height: from the shore to there they grow." },
    Tuned { name: "shore wander", default: 1.0, range: (0.0, 4.0), page: Page::Generation, what: "How far the line the hills begin at wanders above and below the shore, in coasts: 0 leaves a level band round every island; more, and hills here stand out of the ocean and there begin well inland." },
    Tuned { name: "shore span (2^)", default: 10.0, range: (4.0, 14.0), page: Page::Generation, what: "How broad that wandering is: 2 to this power in cells between the noise's points." },
    Tuned { name: "grass cover", default: 0.95, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts as grass." },
    Tuned { name: "patch size (2^)", default: 8.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a patch of grass or of dirt is: 2 to this power, in cells." },
    Tuned { name: "patch detail", default: 0.6, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the patches' edges are: how much finer noise counts beside the patches'." },
    Tuned { name: "scatter", default: 0.05, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: grass scattered cell by cell, not in patches." },
    Tuned { name: "tree cover", default: 0.06, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts with a tree." },
    Tuned { name: "tree patch (2^)", default: 7.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a wood is: 2 to this power, in cells." },
    Tuned { name: "tree detail", default: 0.8, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the woods' edges are." },
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
