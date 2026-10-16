//! What is tuned by eye: the numbers a window's sliders set, by name and
//! place, a value ([`Tuning`]) handed about (`docs/utilities.md`, "What
//! is tuned by eye").

use crate::settings::Settings;
use std::sync::OnceLock;

/// One number to tune, as the sliders' file has it.
pub struct TunedNumber {
    /// Its name, as shown and as saved.
    pub setting_name: &'static str,
    /// Its line in the sliders' file: its group's sliders are in their
    /// lines' order.
    pub line: usize,
    /// The least and the most a slider sets it to.
    pub slider_range: (f32, f32),
    /// The group of sliders it is in.
    pub slider_group: SliderGroup,
    /// Whether it is a toggle: on or off, its range's ends, set by a
    /// click and not dragged.
    pub toggle: bool,
    /// What it does, said to whoever rests the pointer on its slider.
    pub what_it_does: &'static str,
}

/// A group of sliders: the numbers tuned together, one group on the
/// screen at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderGroup {
    /// The near view's shading.
    NearViewShading,
    /// The world's size, whether it is forced hot, whether its camera
    /// loads superchunks, and its sheep.
    World,
    /// The ocean and the land: their levels and their shares.
    OceanAndLand,
    /// The mesh's lines: how they are blended and bent.
    Lines,
    /// The finer meshes on the land.
    FinerMeshes,
    /// The grass a world starts with.
    StartingGrass,
    /// The trees a world starts with.
    Trees,
}

/// The groups, in the order the menu lists them: those only of
/// setting a world up last.
pub const SLIDER_GROUPS: [SliderGroup; 7] = [SliderGroup::NearViewShading, SliderGroup::OceanAndLand, SliderGroup::Lines, SliderGroup::FinerMeshes, SliderGroup::StartingGrass, SliderGroup::Trees, SliderGroup::World];

impl SliderGroup {
    /// Its name, as the menu has it.
    pub const fn menu_name(self) -> &'static str {
        match self {
            Self::NearViewShading => "shading",
            Self::World => "world",
            Self::OceanAndLand => "ocean and land",
            Self::Lines => "mesh lines",
            Self::FinerMeshes => "finer meshes",
            Self::StartingGrass => "grass",
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
    CAST_SHADOW "shadow",
    RELIEF_STRENGTH "relief",
    GROUND_TEXTURE "texture",
    OCEAN_FLOOR "ocean floor level",
    OCEAN_LEVEL "ocean level",
    HIGHEST_LAND "highest land",
    OCEAN_SHARE "ocean share",
    COAST_BREADTH "coast breadth",
    COAST_LOWNESS "coast lowness",
    LAND_CLUMPING "clumping",
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
    SHEEP_PER_SUPERCHUNK "sheep a superchunk",
}

/// The sliders, as written by hand, in CSV: a row each -- its number's name,
/// the least and the most its knob reaches, its group, whether it is a
/// slider or a toggle, what it does.
const SLIDERS: &str = include_str!("../sliders.csv");

/// The `number_index`-th number, as the sliders' file has it, which has
/// every one.
pub fn tuned_number(number_index: usize) -> &'static TunedNumber {
    static TUNED_NUMBERS: OnceLock<[TunedNumber; NAMES.len()]> = OnceLock::new();
    &TUNED_NUMBERS.get_or_init(|| {
        // Read once and kept as long as the program runs: what a slider does is said from it.
        let slider_rows: &'static [Vec<String>] = crate::csv::rows_named(SLIDERS).leak();
        NAMES.map(|setting_name| {
            let (line, slider_row) = slider_rows.iter().enumerate().find(|(_, candidate_row)| candidate_row[0] == setting_name).expect("every number has a row in the sliders' file");
            let [_, least_text, most_text, group_name, slider_kind, what_it_does] = &slider_row[..] else {
                panic!("a slider's row has its name, its range, its group, its kind and what it does");
            };
            let range_ends = [least_text, most_text].map(|end_text| end_text.parse().expect("a range's end is a number"));
            let toggle = match slider_kind.as_str() {
                "toggle" => true,
                "slider" => false,
                unknown_kind => panic!("a slider's kind is `slider` or `toggle`, not `{unknown_kind}`"),
            };
            TunedNumber { setting_name, line, slider_range: (range_ends[0], range_ends[1]), toggle, slider_group: SLIDER_GROUPS.into_iter().find(|listed_group| listed_group.menu_name() == group_name).expect("a slider's group is one of the groups"), what_it_does }
        })
    })[number_index]
}

/// The numbers, read together, each at its place.
pub type Tuning = [f32; NAMES.len()];

/// What the `number_index`-th number is unless set: what the default
/// settings have for it, which have every one.
pub fn unless_set(number_index: usize) -> f32 {
    static DEFAULT_TUNING: OnceLock<Tuning> = OnceLock::new();
    DEFAULT_TUNING.get_or_init(|| {
        let default_values = Settings::default_settings();
        NAMES.map(|setting_name| default_values.value_as(setting_name).expect("every number tuned is in the default settings"))
    })[number_index]
}

/// Every number as the default settings have it.
pub fn default_tuning() -> Tuning {
    std::array::from_fn(unless_set)
}

/// Every number as the machine's settings have it, or its default.
pub fn load_from_settings() -> Tuning {
    let machine_settings = Settings::read_machine();
    std::array::from_fn(|number_index| value_or_default(number_index, machine_settings.value_as(NAMES[number_index]).unwrap_or(f32::NAN)))
}

/// What the `number_index`-th number is set to by `value` -- anything: its
/// range is only what its slider reaches, and a value typed may pass
/// it -- or its default if `value` is no number.
pub fn value_or_default(number_index: usize, value: f32) -> f32 {
    if value.is_finite() { value } else { unless_set(number_index) }
}

/// Keeps `tuning` for the next run, in the machine's settings: lines
/// not these numbers' are left as they are. A failure is let pass:
/// they are then only not kept.
pub fn save_to_settings(tuning: &Tuning) {
    let mut machine_settings = Settings::read_machine();
    for (setting_name, value) in NAMES.into_iter().zip(tuning) {
        machine_settings.set(setting_name, Some(format!("{value:.3}")));
    }
    _ = machine_settings.write();
}
