//! What is tuned by eye: the numbers a window's sliders set, in
//! groups -- the near view's shading, which a painter reads each frame;
//! how a world is generated, read when one is made, or made again
//! while it runs; and what a world is set up with, read only when one
//! is made ([`Group::setup_only`]). The numbers are a value ([`Tuning`]), held by whoever
//! sets them and handed to whoever reads them: nothing of them is
//! held here.
//!
//! Only their names and places are here. What a slider reaches, its
//! group and what it does are in the sliders' file (`sliders.csv`, at
//! the crate's root), written by hand; what each number is unless set
//! is in the default settings ([`crate::settings`]), and what it was
//! last set to is kept in the machine's, a line each.

use crate::settings::Settings;
use std::sync::OnceLock;

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
    /// Whether it is a toggle: on or off, its range's ends, set by a
    /// click and not dragged.
    pub toggle: bool,
    /// What it does, said to whoever rests the pointer on its slider.
    pub what: &'static str,
}

/// A group of sliders: the numbers tuned together, one group on the
/// screen at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// The near view's shading.
    Shading,
    /// The world's size, whether it is forced hot, whether its camera
    /// loads superchunks, and its sheep.
    World,
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
}

/// The groups, in the order the menu lists them: those only of
/// setting a world up last.
pub const GROUPS: [Group; 7] = [Group::Shading, Group::Land, Group::Lines, Group::Finer, Group::Grass, Group::Trees, Group::World];

impl Group {
    /// Its name, as the menu has it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shading => "shading",
            Self::World => "world",
            Self::Land => "ocean and land",
            Self::Lines => "mesh lines",
            Self::Finer => "finer meshes",
            Self::Grass => "grass",
            Self::Trees => "trees",
        }
    }

    /// Whether it is only of setting a world up -- read when one is
    /// made, and shown only then: the world's own group. The shading is
    /// shown while a world runs, and so is how it is generated, the
    /// world made again from its start as it changes.
    pub const fn setup_only(self) -> bool {
        matches!(self, Self::World)
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
    WORLD_SIDE "world side",
    FORCED_HOT "forced hot",
    CAMERA_LOADS "camera loads",
    SHEEP "sheep a superchunk",
}

/// The sliders, as written by hand, in CSV: a row each -- its number's name,
/// the least and the most its knob reaches, its group, whether it is a
/// slider or a toggle, what it does.
const SLIDERS: &str = include_str!("../sliders.csv");

/// The `index`-th number, as the sliders' file has it, which has
/// every one.
pub fn tuned(index: usize) -> &'static Tuned {
    static TUNED: OnceLock<[Tuned; NAMES.len()]> = OnceLock::new();
    &TUNED.get_or_init(|| {
        // Read once and kept as long as the program runs: what a slider does is said from it.
        let rows: &'static [Vec<String>] = crate::csv::rows_named(SLIDERS).leak();
        NAMES.map(|name| {
            let (line, row) = rows.iter().enumerate().find(|(_, row)| row[0] == name).expect("every number has a row in the sliders' file");
            let [_, least, most, group, kind, what] = &row[..] else {
                panic!("a slider's row has its name, its range, its group, its kind and what it does");
            };
            let range = [least, most].map(|end| end.parse().expect("a range's end is a number"));
            let toggle = match kind.as_str() {
                "toggle" => true,
                "slider" => false,
                other => panic!("a slider's kind is `slider` or `toggle`, not `{other}`"),
            };
            Tuned { name, line, range: (range[0], range[1]), toggle, group: GROUPS.into_iter().find(|listed| listed.name() == group).expect("a slider's group is one of the groups"), what }
        })
    })[index]
}

/// The numbers, read together, each at its place.
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

/// Every number as the default settings have it.
pub fn defaults() -> Tuning {
    std::array::from_fn(unless_set)
}

/// Every number as the machine's settings have it, or its default.
pub fn kept() -> Tuning {
    let kept = Settings::read();
    std::array::from_fn(|index| settled(index, kept.number(NAMES[index]).unwrap_or(f32::NAN)))
}

/// What the `index`-th number is set to by `value` -- anything: its
/// range is only what its slider reaches, and a value typed may pass
/// it -- or its default if `value` is no number.
pub fn settled(index: usize, value: f32) -> f32 {
    if value.is_finite() { value } else { unless_set(index) }
}

/// Keeps `tuning` for the next run, in the machine's settings: lines
/// not these numbers' are left as they are. A failure is let pass:
/// they are then only not kept.
pub fn keep(tuning: &Tuning) {
    let mut kept = Settings::read();
    for (name, value) in NAMES.into_iter().zip(tuning) {
        kept.set(name, Some(format!("{value:.3}")));
    }
    _ = kept.write();
}
