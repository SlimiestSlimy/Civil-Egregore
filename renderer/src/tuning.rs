//! What the near view's shading is tuned by: a few numbers the window's
//! sliders ([`crate::sliders`]) set and the painter reads, each frame,
//! with no lock between them. They are kept from one run to the next
//! ([`path`]), so what was found by eye can be read back and written
//! into the code as the defaults.

use std::fs::{create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// One number to tune.
pub struct Tuned {
    /// Its name, as shown and as saved.
    pub name: &'static str,
    /// What it is unless set.
    pub default: f32,
    /// The least and the most a slider sets it to.
    pub range: (f32, f32),
}

/// How much lighter a step's line towards the sun is.
pub const STEP_LIGHT: usize = 0;
/// How much darker a step's line away from the sun is.
pub const STEP_DARK: usize = 1;
/// How much darker the foot of a wall in its own shade is.
pub const WALL_SHADE: usize = 2;
/// How much of that its band has lost at its far edge.
pub const WALL_FADE: usize = 3;
/// How much darker the foot of a wall the sun is on is.
pub const WALL_FOOT: usize = 4;
/// How much lighter the line beside that foot is.
pub const WALL_BRIGHT: usize = 5;
/// How much lighter the lip over a wall is.
pub const LIP: usize = 6;
/// How much darker a cast shadow is.
pub const SHADOW: usize = 7;
/// How much of the slopes' light and the heights' tint is shown.
pub const RELIEF: usize = 8;
/// How much the ground's pixels differ by lot.
pub const TEXTURE: usize = 9;

/// The numbers, in the order above.
pub const TUNED: [Tuned; 10] = [
    Tuned { name: "step light", default: 0.16, range: (0.0, 0.6) },
    Tuned { name: "step dark", default: 0.26, range: (0.0, 0.8) },
    Tuned { name: "wall shade", default: 0.75, range: (0.0, 1.0) },
    Tuned { name: "wall fade", default: 0.45, range: (0.0, 1.0) },
    Tuned { name: "wall foot", default: 0.55, range: (0.0, 1.0) },
    Tuned { name: "wall bright", default: 0.3, range: (0.0, 0.8) },
    Tuned { name: "lip", default: 0.32, range: (0.0, 0.8) },
    Tuned { name: "shadow", default: 0.26, range: (0.0, 0.7) },
    Tuned { name: "relief", default: 1.0, range: (0.0, 3.0) },
    Tuned { name: "texture", default: 1.0, range: (0.0, 3.0) },
];

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

/// Sets the `index`-th number, within its range.
pub fn set(index: usize, value: f32) {
    let (least, most) = TUNED[index].range;
    VALUES[index].store(value.clamp(least, most).to_bits(), Ordering::Relaxed);
}

/// Keeps the numbers for the next run. A failure is let pass: they are
/// then only not kept.
pub fn keep() {
    let lines: String = TUNED.iter().zip(now()).map(|(tuned, value)| format!("{} = {value:.3}\n", tuned.name)).collect();
    let path = path();
    _ = path.parent().map(create_dir_all);
    _ = write(path, lines);
}
