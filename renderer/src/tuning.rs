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

/// The numbers, in the order above; the shading's defaults are what was
/// found by eye with the sliders.
pub const TUNED: [Tuned; 17] = [
    Tuned { name: "step light", default: 0.35, range: (0.0, 1.0), page: Page::Shading },
    Tuned { name: "step dark", default: 0.35, range: (0.0, 0.8), page: Page::Shading },
    Tuned { name: "wall shade", default: 0.49, range: (0.0, 1.0), page: Page::Shading },
    Tuned { name: "wall lit", default: 0.55, range: (0.0, 1.0), page: Page::Shading },
    Tuned { name: "wall fade", default: 0.8, range: (0.0, 1.5), page: Page::Shading },
    Tuned { name: "shadow", default: 0.4, range: (0.0, 0.8), page: Page::Shading },
    Tuned { name: "relief", default: 0.7, range: (0.0, 3.0), page: Page::Shading },
    Tuned { name: "texture", default: 2.0, range: (0.0, 4.0), page: Page::Shading },
    Tuned { name: "hills (512)", default: 150.0, range: (0.0, 255.0), page: Page::Generation },
    Tuned { name: "ridges (128)", default: 75.0, range: (0.0, 255.0), page: Page::Generation },
    Tuned { name: "bumps (32)", default: 24.0, range: (0.0, 255.0), page: Page::Generation },
    Tuned { name: "roughness (8)", default: 6.0, range: (0.0, 255.0), page: Page::Generation },
    Tuned { name: "height span", default: 255.0, range: (0.0, 255.0), page: Page::Generation },
    Tuned { name: "grass cover", default: 0.38, range: (0.0, 1.0), page: Page::Generation },
    Tuned { name: "patch size (2^)", default: 6.0, range: (1.0, 10.0), page: Page::Generation },
    Tuned { name: "patch detail", default: 0.3, range: (0.0, 2.0), page: Page::Generation },
    Tuned { name: "scatter", default: 0.1, range: (0.0, 2.0), page: Page::Generation },
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

/// Sets the `index`-th number, within its range; the world is to be
/// generated again if it is one of generation's and has changed.
pub fn set(index: usize, value: f32) {
    let (least, most) = TUNED[index].range;
    let bits = value.clamp(least, most).to_bits();
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
