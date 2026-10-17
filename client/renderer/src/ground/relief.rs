//! Relief: what a height does to a colour, the same wherever the
//! world is drawn -- slope light, height tint, water's light, sand and
//! foam (`docs/renderer.md`, "Height, from straight above").

/// The rise a cell from which a slope is lit, or darkened, no more.
const STEEPEST_RISE: f32 = 64.0;
/// The rise a cell under which a slope hardly shows.
const GENTLEST_RISE: f32 = 0.05;
/// How much lighter the steepest slope facing the sun is.
const TOWARDS_SUN: f32 = 0.30;
/// How much darker the steepest slope facing away from it is.
const AWAY_FROM_SUN: f32 = 0.38;
/// How much darker the steepest slope across the sun is: it would
/// otherwise show as level ground does.
const ACROSS_SUN: f32 = 0.12;

/// The light on ground rising `rise_across` and `rise_down` heights a cell: 1
/// level ground's, more facing the sun, which is to the top left.
pub fn slope_light(rise_across: f32, rise_down: f32) -> f32 {
    let total_rise = rise_across.hypot(rise_down);
    if total_rise == 0.0 {
        return 1.0;
    }
    let steepness_factor = ((1.0 + total_rise / GENTLEST_RISE).ln() / (1.0 + STEEPEST_RISE / GENTLEST_RISE).ln()).min(1.0);
    let sun_facing = (rise_across + rise_down) / (total_rise * std::f32::consts::SQRT_2);
    1.0 + steepness_factor * (if sun_facing > 0.0 { TOWARDS_SUN } else { AWAY_FROM_SUN } * sun_facing - ACROSS_SUN * (1.0 - sun_facing.abs()))
}

/// The colour laid over the highest ground.
pub const PALE_COLOUR: [u8; 3] = [240, 240, 244];
/// The colour laid over the land beside water.
pub const SAND_COLOUR: [u8; 3] = [226, 208, 150];
/// The colour laid over the water beside land.
pub const FOAM_COLOUR: [u8; 3] = [214, 238, 244];
/// How much of the land right beside water is sand.
pub const SAND_MOST: f32 = 0.85;
/// How much of the water right beside land is foam.
pub const FOAM_MOST: f32 = 0.55;

/// The tint at each share of the way from the ocean's level to the
/// highest land: the share, what a colour is multiplied by, and how
/// much of it is [`PALE_COLOUR`].
const HEIGHT_TINTS: [(f32, [f32; 3], f32); 6] = [
    (0.00, [0.70, 0.80, 0.74], 0.00),
    (0.12, [0.86, 0.93, 0.78], 0.00),
    (0.30, [1.08, 1.06, 0.78], 0.00),
    (0.50, [1.30, 1.10, 0.82], 0.10),
    (0.75, [1.30, 1.02, 1.00], 0.40),
    (1.00, [1.20, 1.10, 1.20], 0.85),
];

/// The bands the tint is in, from the ocean's level to the highest
/// land: ground a band higher is told from the one under it by its
/// tint alone, level as both may be.
const TINT_BANDS: f32 = 40.0;

/// The tint of ground `height_share` of the way from the ocean's level to the
/// highest land: what its colour is multiplied by, red, green and
/// blue, and how much of it is [`PALE_COLOUR`].
pub fn height_tint(height_share: f32) -> ([f32; 3], f32) {
    let banded_share = (height_share.clamp(0.0, 1.0) * TINT_BANDS).floor() / TINT_BANDS;
    let upper_tint = HEIGHT_TINTS.iter().position(|&(tint_share, ..)| tint_share >= banded_share).unwrap_or(HEIGHT_TINTS.len() - 1).max(1);
    let ((lower_share, lower_factor, low_pale), (upper_share, upper_factor, high_pale)) = (HEIGHT_TINTS[upper_tint - 1], HEIGHT_TINTS[upper_tint]);
    let share_along = (banded_share - lower_share) / (upper_share - lower_share);
    (std::array::from_fn(|colour_channel| lower_factor[colour_channel] + (upper_factor[colour_channel] - lower_factor[colour_channel]) * share_along), low_pale + (high_pale - low_pale) * share_along)
}

/// What water's colour is multiplied by over the shallowest water.
const SHALLOWEST_FACTORS: [f32; 3] = [1.35, 1.52, 1.30];
/// What it is multiplied by over the deepest.
const DEEPEST_FACTORS: [f32; 3] = [0.34, 0.40, 0.58];
/// The depth the water's light halves its way down by, about.
const SHALLOWS_DEPTH: f32 = 12.0;
/// The bands the water's light is in, from the shallowest to the deepest.
const WATER_BANDS: f32 = 14.0;

/// What water's colour is multiplied by over ground `water_depth` under it,
/// where the deepest there is is `deepest_depth`: light over the shallows,
/// dark over the deep, in bands -- a doubling of the depth as far down
/// the bands wherever it is.
pub fn water_light(water_depth: f32, deepest_depth: f32) -> [f32; 3] {
    let depth_share = ((1.0 + water_depth.max(0.0) / SHALLOWS_DEPTH).ln() / (1.0 + deepest_depth.max(1.0) / SHALLOWS_DEPTH).ln()).min(1.0);
    let banded_depth_share = (depth_share * WATER_BANDS).round() / WATER_BANDS;
    std::array::from_fn(|colour_channel| SHALLOWEST_FACTORS[colour_channel] + (DEEPEST_FACTORS[colour_channel] - SHALLOWEST_FACTORS[colour_channel]) * banded_depth_share)
}

/// `land_tint` on ground `sand_share` of which is sand: sand is its own colour at
/// any height.
pub fn tint_on_sand(land_tint: [f32; 3], sand_share: f32) -> [f32; 3] {
    land_tint.map(|tint_channel| tint_channel + (1.0 - tint_channel) * sand_share / SAND_MOST)
}

/// `under_colour` with `part` of it `over_colour`.
pub fn laid_over(under_colour: [u8; 3], over_colour: [u8; 3], part: f32) -> [u8; 3] {
    std::array::from_fn(|colour_channel| (under_colour[colour_channel] as f32 + (over_colour[colour_channel] as f32 - under_colour[colour_channel] as f32) * part).round() as u8)
}
