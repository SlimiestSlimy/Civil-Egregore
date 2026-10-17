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

/// The numbers' names, as shown and as saved, each at its place. The
/// places are for the code alone, which reads a number by its constant
/// ([`place_of`]): no file is read by them, nor in their order.
pub const NAMES: [&str; 40] = [
    "step light",
    "step dark",
    "wall shade",
    "wall lit",
    "wall fade",
    "shadow",
    "relief",
    "texture",
    "ocean floor level",
    "ocean level",
    "highest land",
    "ocean share",
    "coast breadth",
    "coast lowness",
    "clumping",
    "vertex spacing (2^)",
    "narrowest blend",
    "widest blend",
    "least sigmoid",
    "most sigmoid",
    "line bending",
    "finer mesh depth",
    "finer mesh share",
    "finer mesh height",
    "finer mesh falloff",
    "weight spread",
    "raised share",
    "grass cover",
    "grass patch size (2^)",
    "grass patch detail",
    "grass scatter",
    "tree cover",
    "tree patch size (2^)",
    "tree patch detail",
    "tree scatter",
    "wall length",
    "world side",
    "forced hot",
    "camera loads",
    "sheep a superchunk",
];

/// The place among [`NAMES`] of the number named `name`: a name not
/// among them fails the build.
const fn place_of(name: &str) -> usize {
    let mut place = 0;
    while place < NAMES.len() {
        if same_text(NAMES[place], name) {
            return place;
        }
        place += 1;
    }
    panic!("a number not among the names")
}

/// Whether `text` and `other_text` are the same, byte for byte: what
/// [`place_of`] compares by while the build runs.
const fn same_text(text: &str, other_text: &str) -> bool {
    let (bytes, other_bytes) = (text.as_bytes(), other_text.as_bytes());
    if bytes.len() != other_bytes.len() {
        return false;
    }
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != other_bytes[at] {
            return false;
        }
        at += 1;
    }
    true
}

/// The place of `step light` among the numbers.
pub const STEP_LIGHT: usize = place_of("step light");
/// The place of `step dark` among the numbers.
pub const STEP_DARK: usize = place_of("step dark");
/// The place of `wall shade` among the numbers.
pub const WALL_SHADE: usize = place_of("wall shade");
/// The place of `wall lit` among the numbers.
pub const WALL_LIT: usize = place_of("wall lit");
/// The place of `wall fade` among the numbers.
pub const WALL_FADE: usize = place_of("wall fade");
/// The place of `shadow` among the numbers.
pub const CAST_SHADOW: usize = place_of("shadow");
/// The place of `relief` among the numbers.
pub const RELIEF_STRENGTH: usize = place_of("relief");
/// The place of `texture` among the numbers.
pub const GROUND_TEXTURE: usize = place_of("texture");
/// The place of `ocean floor level` among the numbers.
pub const OCEAN_FLOOR: usize = place_of("ocean floor level");
/// The place of `ocean level` among the numbers.
pub const OCEAN_LEVEL: usize = place_of("ocean level");
/// The place of `highest land` among the numbers.
pub const HIGHEST_LAND: usize = place_of("highest land");
/// The place of `ocean share` among the numbers.
pub const OCEAN_SHARE: usize = place_of("ocean share");
/// The place of `coast breadth` among the numbers.
pub const COAST_BREADTH: usize = place_of("coast breadth");
/// The place of `coast lowness` among the numbers.
pub const COAST_LOWNESS: usize = place_of("coast lowness");
/// The place of `clumping` among the numbers.
pub const LAND_CLUMPING: usize = place_of("clumping");
/// The place of `vertex spacing (2^)` among the numbers.
pub const VERTEX_SPACING: usize = place_of("vertex spacing (2^)");
/// The place of `narrowest blend` among the numbers.
pub const NARROWEST_BLEND: usize = place_of("narrowest blend");
/// The place of `widest blend` among the numbers.
pub const WIDEST_BLEND: usize = place_of("widest blend");
/// The place of `least sigmoid` among the numbers.
pub const LEAST_SIGMOID: usize = place_of("least sigmoid");
/// The place of `most sigmoid` among the numbers.
pub const MOST_SIGMOID: usize = place_of("most sigmoid");
/// The place of `line bending` among the numbers.
pub const LINE_BENDING: usize = place_of("line bending");
/// The place of `finer mesh depth` among the numbers.
pub const FINER_DEPTH: usize = place_of("finer mesh depth");
/// The place of `finer mesh share` among the numbers.
pub const FINER_SHARE: usize = place_of("finer mesh share");
/// The place of `finer mesh height` among the numbers.
pub const FINER_HEIGHT: usize = place_of("finer mesh height");
/// The place of `finer mesh falloff` among the numbers.
pub const FINER_FALL: usize = place_of("finer mesh falloff");
/// The place of `weight spread` among the numbers.
pub const WEIGHT_SPREAD: usize = place_of("weight spread");
/// The place of `raised share` among the numbers.
pub const RAISED_SHARE: usize = place_of("raised share");
/// The place of `grass cover` among the numbers.
pub const GRASS_COVER: usize = place_of("grass cover");
/// The place of `grass patch size (2^)` among the numbers.
pub const GRASS_PATCH: usize = place_of("grass patch size (2^)");
/// The place of `grass patch detail` among the numbers.
pub const GRASS_DETAIL: usize = place_of("grass patch detail");
/// The place of `grass scatter` among the numbers.
pub const GRASS_SCATTER: usize = place_of("grass scatter");
/// The place of `tree cover` among the numbers.
pub const TREE_COVER: usize = place_of("tree cover");
/// The place of `tree patch size (2^)` among the numbers.
pub const TREE_PATCH: usize = place_of("tree patch size (2^)");
/// The place of `tree patch detail` among the numbers.
pub const TREE_DETAIL: usize = place_of("tree patch detail");
/// The place of `tree scatter` among the numbers.
pub const TREE_SCATTER: usize = place_of("tree scatter");
/// The place of `wall length` among the numbers.
pub const WALL_LENGTH: usize = place_of("wall length");
/// The place of `world side` among the numbers.
pub const WORLD_SIDE: usize = place_of("world side");
/// The place of `forced hot` among the numbers.
pub const FORCED_HOT: usize = place_of("forced hot");
/// The place of `camera loads` among the numbers.
pub const CAMERA_LOADS: usize = place_of("camera loads");
/// The place of `sheep a superchunk` among the numbers.
pub const SHEEP_PER_SUPERCHUNK: usize = place_of("sheep a superchunk");

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
