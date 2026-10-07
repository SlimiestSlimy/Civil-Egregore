//! What is tuned by eye: the numbers the window's sliders
//! ([`crate::sliders`]) set, in groups -- the near view's shading,
//! which a painter reads each frame, and in the lab how the world is
//! generated, its seed drawn again among them ([`reseed`]) -- with no
//! lock between them.
//!
//! Only their names and places are here. What a slider reaches, its
//! group and what it does are in the sliders' file (`sliders.csv`, at
//! the crate's root), written by hand; what each number is unless set
//! is in the default settings (`utilities::settings`), and what it was
//! last set to is kept in the machine's, a line each.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use utilities::hash::mix;
use utilities::settings::Settings;

/// One number to tune, as the sliders' file has it.
pub struct Tuned {
    /// Its name, as shown and as saved.
    pub name: &'static str,
    /// Its line in the sliders' file: its group's sliders are in their
    /// lines' order.
    pub line: usize,
    /// The least and the most a slider sets it to.
    pub range: (f32, f32),
    /// The group of sliders it is in.
    pub group: Group,
    /// What it does, said to whoever rests the pointer on its slider.
    pub what: &'static str,
}

/// A group of sliders: the numbers tuned together, one group on the
/// screen at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// The near view's shading.
    Shading,
    /// The ocean and the land: their levels and their shares.
    Land,
    /// The mesh's lines: how they are blended and bent.
    Lines,
    /// The finer meshes on the land.
    Finer,
    /// The grass a world starts with.
    Grass,
    /// The trees a world starts with.
    Trees,
    /// The sheep a world starts with.
    Sheep,
}

/// The groups, in the order the menu lists them.
pub const GROUPS: [Group; 7] = [Group::Shading, Group::Land, Group::Lines, Group::Finer, Group::Grass, Group::Trees, Group::Sheep];

impl Group {
    /// Its name, as the menu has it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shading => "shading",
            Self::Land => "ocean and land",
            Self::Lines => "mesh lines",
            Self::Finer => "finer meshes",
            Self::Grass => "grass",
            Self::Trees => "trees",
            Self::Sheep => "sheep",
        }
    }

    /// Whether it is shown only in the lab: every group but the near
    /// view's shading, which always is.
    pub const fn lab_only(self) -> bool {
        !matches!(self, Self::Shading)
    }
}

/// Names the numbers and their places among them: a constant each,
/// and [`NAMES`]. The places are for the code alone, which reads a
/// number by its constant: no file is read by them, nor in their
/// order.
macro_rules! places {
    ($($place:ident $name:literal,)*) => {
        /// The numbers' names, as shown and as saved, each at its place.
        pub const NAMES: [&str; [$($name),*].len()] = [$($name),*];
        places!(0; $($place $name,)*);
    };
    ($at:expr; $place:ident $name:literal, $($rest:tt)*) => {
        #[doc = concat!("The place of `", $name, "` among the numbers.")]
        pub const $place: usize = $at;
        places!($at + 1; $($rest)*);
    };
    ($at:expr;) => {};
}

places! {
    STEP_LIGHT "step light",
    STEP_DARK "step dark",
    WALL_SHADE "wall shade",
    WALL_LIT "wall lit",
    WALL_FADE "wall fade",
    SHADOW "shadow",
    RELIEF "relief",
    TEXTURE "texture",
    OCEAN_FLOOR "ocean floor level",
    OCEAN_LEVEL "ocean level",
    HIGHEST_LAND "highest land",
    OCEAN_SHARE "ocean share",
    COAST_BREADTH "coast breadth",
    COAST_LOWNESS "coast lowness",
    CLUMPING "clumping",
    VERTEX_SPACING "vertex spacing (2^)",
    NARROWEST_BLEND "narrowest blend",
    WIDEST_BLEND "widest blend",
    LEAST_SIGMOID "least sigmoid",
    MOST_SIGMOID "most sigmoid",
    LINE_BENDING "line bending",
    FINER_DEPTH "finer mesh depth",
    FINER_SHARE "finer mesh share",
    FINER_HEIGHT "finer mesh height",
    FINER_FALL "finer mesh falloff",
    WEIGHT_SPREAD "weight spread",
    RAISED_SHARE "raised share",
    GRASS_COVER "grass cover",
    GRASS_PATCH "grass patch size (2^)",
    GRASS_DETAIL "grass patch detail",
    GRASS_SCATTER "grass scatter",
    TREE_COVER "tree cover",
    TREE_PATCH "tree patch size (2^)",
    TREE_DETAIL "tree patch detail",
    TREE_SCATTER "tree scatter",
    WALL_LENGTH "wall length",
    SHEEP "sheep a superchunk",
}

/// The sliders, as written by hand, in CSV: a row each -- its number's name,
/// the least and the most its knob reaches, its group, what it does.
const SLIDERS: &str = include_str!("../sliders.csv");

/// The `index`-th number, as the sliders' file has it, which has
/// every one.
pub fn tuned(index: usize) -> &'static Tuned {
    static TUNED: OnceLock<[Tuned; NAMES.len()]> = OnceLock::new();
    &TUNED.get_or_init(|| {
        // Read once and kept as long as the program runs: what a slider does is said from it.
        let rows: &'static [Vec<String>] = utilities::csv::rows_named(SLIDERS).leak();
        NAMES.map(|name| {
            let (line, row) = rows.iter().enumerate().find(|(_, row)| row[0] == name).expect("every number has a row in the sliders' file");
            let [_, least, most, group, what] = &row[..] else {
                panic!("a slider's row has its name, its range, its group and what it does");
            };
            let range = [least, most].map(|end| end.parse().expect("a range's end is a number"));
            Tuned { name, line, range: (range[0], range[1]), group: GROUPS.into_iter().find(|listed| listed.name() == group).expect("a slider's group is one of the groups"), what }
        })
    })[index]
}

/// Counts the changes to how the world is generated: what was made
/// under an earlier count is made again. Not a generation itself --
/// `server::Generation`, the recipe -- only a count of when one last
/// changed.
static REVISION: AtomicU64 = AtomicU64::new(0);

/// How many times how the world is generated has changed.
pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

/// Says that how the world is generated has changed.
pub fn revise() {
    REVISION.fetch_add(1, Ordering::Relaxed);
}

/// The seed drawn last, 0 while none was.
static SEED_DRAWN: AtomicU64 = AtomicU64::new(0);

/// The seed drawn last ([`reseed`]), 0 while none was: the world is
/// then generated from the run's own.
pub fn seed_drawn() -> u64 {
    SEED_DRAWN.load(Ordering::Relaxed)
}

/// Draws a new seed, off the clock, never 0: the world is to be
/// generated again.
pub fn reseed() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos() as u64);
    SEED_DRAWN.store(mix(now).max(1), Ordering::Relaxed);
    revise();
}

/// The numbers as they are now, each a float's bits.
static VALUES: [AtomicU32; NAMES.len()] = [const { AtomicU32::new(0) }; NAMES.len()];

/// The numbers, read together.
pub type Tuning = [f32; NAMES.len()];

/// What the `index`-th number is unless set: what the default
/// settings have for it, which have every one.
pub fn unless_set(index: usize) -> f32 {
    static DEFAULTS: OnceLock<Tuning> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        let defaults = Settings::defaults();
        NAMES.map(|name| defaults.number(name).expect("every number tuned is in the default settings"))
    })[index]
}

/// Sets every number to what the machine's settings have for it, or
/// to its default.
pub fn start() {
    let kept = Settings::read();
    for (index, name) in NAMES.into_iter().enumerate() {
        set(index, kept.number(name).unwrap_or(unless_set(index)));
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
    let bits = if value.is_finite() { value } else { unless_set(index) }.to_bits();
    if VALUES[index].swap(bits, Ordering::Relaxed) != bits {
        revise();
    }
}

/// Keeps the numbers for the next run, in the machine's settings:
/// lines not these numbers' are left as they are. A failure is let
/// pass: they are then only not kept.
pub fn keep() {
    let mut kept = Settings::read();
    for (name, value) in NAMES.into_iter().zip(now()) {
        kept.set(name, Some(format!("{value:.3}")));
    }
    _ = kept.write();
}
