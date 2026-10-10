//! Relief: what a height does to a colour, the same wherever the
//! world is drawn -- slope light, height tint, water's light, sand and
//! foam (`docs/renderer.md`, "Height, from straight above").

/// The rise a cell from which a slope is lit, or darkened, no more.
const STEEPEST: f32 = 64.0;
/// The rise a cell under which a slope hardly shows.
const GENTLEST: f32 = 0.05;
/// How much lighter the steepest slope facing the sun is.
const TOWARDS_SUN: f32 = 0.30;
/// How much darker the steepest slope facing away from it is.
const AWAY_FROM_SUN: f32 = 0.38;
/// How much darker the steepest slope across the sun is: it would
/// otherwise show as level ground does.
const ACROSS_SUN: f32 = 0.12;

/// The light on ground rising `across` and `down` heights a cell: 1
/// level ground's, more facing the sun, which is to the top left.
pub fn slope_light(across: f32, down: f32) -> f32 {
    let rise = across.hypot(down);
    if rise == 0.0 {
        return 1.0;
    }
    let steep = ((1.0 + rise / GENTLEST).ln() / (1.0 + STEEPEST / GENTLEST).ln()).min(1.0);
    let facing = (across + down) / (rise * std::f32::consts::SQRT_2);
    1.0 + steep * (if facing > 0.0 { TOWARDS_SUN } else { AWAY_FROM_SUN } * facing - ACROSS_SUN * (1.0 - facing.abs()))
}

/// The colour laid over the highest ground.
pub const PALE: [u8; 3] = [240, 240, 244];
/// The colour laid over the land beside water.
pub const SAND: [u8; 3] = [226, 208, 150];
/// The colour laid over the water beside land.
pub const FOAM: [u8; 3] = [214, 238, 244];
/// How much of the land right beside water is sand.
pub const SAND_MOST: f32 = 0.85;
/// How much of the water right beside land is foam.
pub const FOAM_MOST: f32 = 0.55;

/// The tint at each share of the way from the ocean's level to the
/// highest land: the share, what a colour is multiplied by, and how
/// much of it is [`PALE`].
const TINTS: [(f32, [f32; 3], f32); 6] = [
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

/// The tint of ground `share` of the way from the ocean's level to the
/// highest land: what its colour is multiplied by, red, green and
/// blue, and how much of it is [`PALE`].
pub fn tint(share: f32) -> ([f32; 3], f32) {
    let share = (share.clamp(0.0, 1.0) * TINT_BANDS).floor() / TINT_BANDS;
    let upper = TINTS.iter().position(|&(at, ..)| at >= share).unwrap_or(TINTS.len() - 1).max(1);
    let ((from, low, low_pale), (to, high, high_pale)) = (TINTS[upper - 1], TINTS[upper]);
    let along = (share - from) / (to - from);
    (std::array::from_fn(|channel| low[channel] + (high[channel] - low[channel]) * along), low_pale + (high_pale - low_pale) * along)
}

/// What water's colour is multiplied by over the shallowest water.
const SHALLOWEST: [f32; 3] = [1.35, 1.52, 1.30];
/// What it is multiplied by over the deepest.
const DEEPEST: [f32; 3] = [0.34, 0.40, 0.58];
/// The depth the water's light halves its way down by, about.
const SHALLOWS: f32 = 12.0;
/// The bands the water's light is in, from the shallowest to the deepest.
const WATER_BANDS: f32 = 14.0;

/// What water's colour is multiplied by over ground `depth` under it,
/// where the deepest there is is `deepest`: light over the shallows,
/// dark over the deep, in bands -- a doubling of the depth as far down
/// the bands wherever it is.
pub fn water_light(depth: f32, deepest: f32) -> [f32; 3] {
    let down = ((1.0 + depth.max(0.0) / SHALLOWS).ln() / (1.0 + deepest.max(1.0) / SHALLOWS).ln()).min(1.0);
    let down = (down * WATER_BANDS).round() / WATER_BANDS;
    std::array::from_fn(|channel| SHALLOWEST[channel] + (DEEPEST[channel] - SHALLOWEST[channel]) * down)
}

/// `tint` on ground `sand` of which is sand: sand is its own colour at
/// any height.
pub fn tint_on_sand(tint: [f32; 3], sand: f32) -> [f32; 3] {
    tint.map(|tint| tint + (1.0 - tint) * sand / SAND_MOST)
}

/// `from` with `part` of it `to`.
pub fn laid(from: [u8; 3], to: [u8; 3], part: f32) -> [u8; 3] {
    std::array::from_fn(|channel| (from[channel] as f32 + (to[channel] as f32 - from[channel] as f32) * part).round() as u8)
}
