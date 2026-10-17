//! The light on the ground: a superchunk's heights and what they do
//! to a cell's colour, seen from straight above with the sun to the
//! top left -- made once and kept, heights never changing
//! (`docs/renderer.md`, "Height, from straight above").

mod levels;
mod light_and_shadow;
pub mod relief;
use levels::GroundAtDetail;
use light_and_shadow::{banded_light, coast_distances, shadow_lines, box_smoothed};
use relief::{slope_light, height_tint, water_light, FOAM_COLOUR, PALE_COLOUR, SAND_COLOUR};

use coordinates::{place_from_cartesian, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashMap;
use protocol::pipe::frame::height_in_frame;
use protocol::pipe::terrain::{AsksTerrain, Height, HeightsAsk, DrawnHeightRange};

/// Cells along a superchunk's side.
pub const SUPERCHUNK_CELLS_A_SIDE: usize = SUPERCHUNK_SIDE_CELLS as usize;
/// Heights the shadow line drops over one cell down the diagonal: a
/// face steeper than this away from the sun is in its own shadow, and
/// casts one a cell long for each so many heights it stands.
const SHADOW_DROP: f32 = 24.0;
/// How far apart the bands of the slopes' light are.
const LIGHT_BAND: f32 = 0.05;
/// Cells from the coast it is looked for no farther than: as wide as
/// it is ever drawn.
pub(crate) const COAST_REACH: f32 = 24.0;
/// Cells the heights are smoothed over, each way, twice.
const SMOOTHED_OVER: usize = 4;
/// Cells kept before a superchunk's top left: as far as a shadow is
/// followed back, and what smoothing needs past that.
const CELLS_KEPT_BEFORE: usize = 138;
/// Cells kept past its bottom right: what smoothing needs, and as far
/// as the coast is looked for.
const CELLS_KEPT_AFTER: usize = COAST_REACH as usize + 2;
/// Cells along the side of the heights worked on.
const WORKED_SIDE_CELLS: usize = CELLS_KEPT_BEFORE + SUPERCHUNK_CELLS_A_SIDE + CELLS_KEPT_AFTER;
/// Cells kept of them past each side of the superchunk: as far as a
/// wall's band is looked for.
pub const KEPT_MARGIN_CELLS: usize = 8;
/// Cells along the side of what is kept of them.
const KEPT_SIDE_CELLS: usize = SUPERCHUNK_CELLS_A_SIDE + 2 * KEPT_MARGIN_CELLS;

/// A cell's light kept in seven bits: this is flat ground's, unshaded.
const LIT_ONE: f32 = 80.0;
/// The bit of a cell's light saying a shadow falls on it.
const SHADOWED_BIT: u8 = 0x80;
/// A colour's factor in a byte: this is one.
pub const FACTOR_ONE: u32 = 128;
/// What a cast shadow does to a colour: darker, and bluer.
pub const CAST_SHADOW_FACTORS: [f32; 3] = [0.66, 0.70, 0.86];
/// The coarsest the ground is drawn: a pixel `2^8` cells a side, a
/// chunk.
pub const COARSEST_DETAIL: usize = 8;
/// The coarsest of the levels dropped with the fine parts.
const FINE_LEVELS: usize = 2;

/// Heights the shadow line drops over one cell down the diagonal.
pub const fn shadow_drop() -> f32 {
    SHADOW_DROP
}

/// What a pixel's colour is drawn through: what it is multiplied by,
/// red, green and blue of [`FACTOR_ONE`], and before that what is laid
/// over it -- which of [`PALE`], [`SAND`] and [`FOAM`] in the last
/// byte's two high bits, how much of it in the six low.
#[derive(Clone, Copy)]
pub struct PixelShade([u8; 4]);

impl PixelShade {
    /// The bits saying how much is laid over.
    const PART_BITS: u32 = 6;
    /// All of it laid over.
    const WHOLE_LAID: u32 = (1 << Self::PART_BITS) - 1;
}

/// `seen_colour` through `pixel_shade`.
pub fn lit_colour(seen_colour: [u8; 3], pixel_shade: PixelShade) -> [u8; 3] {
    let PixelShade([red_factor, green_factor, blue_factor, laid_over_byte]) = pixel_shade;
    let (laid_colour, part) = ([PALE_COLOUR, SAND_COLOUR, FOAM_COLOUR][(laid_over_byte >> PixelShade::PART_BITS) as usize % 3], laid_over_byte as u32 & PixelShade::WHOLE_LAID);
    let colour_factors = [red_factor, green_factor, blue_factor];
    std::array::from_fn(|colour_channel| ((seen_colour[colour_channel] as u32 * (PixelShade::WHOLE_LAID - part) + laid_colour[colour_channel] as u32 * part) / PixelShade::WHOLE_LAID * colour_factors[colour_channel] as u32 / FACTOR_ONE).min(255) as u8)
}

/// What is kept of a superchunk only while it is seen from near: 3 MiB
/// and the two finest levels.
pub struct FineGround {
    /// Its cells' heights, and [`KEPT_MARGIN_CELLS`] cells more all round, row by row.
    heights: Vec<Height>,
    /// How high the shadow line stands over each, laid out as the
    /// heights.
    lines: Vec<f32>,
    /// Each cell's light, of [`LIT_ONE`] -- its slope's -- and
    /// whether a shadow falls on it ([`SHADOWED_BIT`]).
    cell_lights: Vec<u8>,
    /// The ocean's level, and the highest land.
    ocean_and_highest: (Height, Height),
    /// The deepest the ocean is.
    deepest_ocean: f32,
}

impl FineGround {
    /// Where the cell `(x, y)` from the top left is kept: [`KEPT_MARGIN_CELLS`]
    /// cells past each edge are.
    const fn offset_of(x: isize, y: isize) -> usize {
        (y + KEPT_MARGIN_CELLS as isize) as usize * KEPT_SIDE_CELLS + (x + KEPT_MARGIN_CELLS as isize) as usize
    }

    /// The height of the cell `(x, y)`.
    pub fn height(&self, x: isize, y: isize) -> Height {
        self.heights[Self::offset_of(x, y)]
    }

    /// How high the shadow line stands over the cell `(x, y)`, in heights.
    pub fn shadow_line(&self, x: isize, y: isize) -> f32 {
        self.lines[Self::offset_of(x, y)]
    }

    /// The light of the cell `(x, y)`: 1 flat ground's, shadows apart.
    pub fn cell_light(&self, x: usize, y: usize) -> f32 {
        (self.cell_lights[y * SUPERCHUNK_CELLS_A_SIDE + x] & !SHADOWED_BIT) as f32 / LIT_ONE
    }

    /// How far the cell `(x, y)` is under the ocean's level, if it is.
    pub fn under_ocean(&self, x: isize, y: isize) -> Option<Height> {
        self.ocean_and_highest.0.checked_sub(self.height(x, y)).filter(|&ocean_depth| ocean_depth > 0)
    }

    /// What a shadow falls on at the cell `(x, y)`: its ground, or the
    /// ocean over it.
    pub fn shadow_surface(&self, x: isize, y: isize) -> Height {
        self.height(x, y).max(self.ocean_and_highest.0)
    }

    /// The deepest the ocean is.
    pub fn deepest_ocean_depth(&self) -> f32 {
        self.deepest_ocean
    }

    /// The tint of the cell `(x, y)`'s height ([`relief::height_tint`]).
    pub fn cell_tint(&self, x: isize, y: isize) -> ([f32; 3], f32) {
        height_tint(height_share(self.height(x, y), self.ocean_and_highest))
    }
}

/// How far `height` is from the ocean's level to the highest land, the
/// two `ocean_and_highest`.
fn height_share(height: Height, ocean_and_highest: (Height, Height)) -> f32 {
    height.saturating_sub(ocean_and_highest.0) as f32 / ocean_and_highest.1.saturating_sub(ocean_and_highest.0).max(1) as f32
}

/// A superchunk's ground.
pub struct SuperchunkGround {
    /// What a pixel `2^detail` cells a side draws its colour through,
    /// a level a detail up to [`COARSEST_DETAIL`]. The two finest are empty
    /// once [`Self::fine_ground`] is dropped.
    pub shades_by_detail: Vec<Vec<PixelShade>>,
    /// What only a near view needs.
    pub fine_ground: Option<FineGround>,
    /// The frame it was last drawn in.
    pub last_used_frame: u64,
}

impl SuperchunkGround {
    /// Drops what only a near view needs.
    pub fn drop_fine_parts(&mut self) {
        self.fine_ground = None;
        self.shades_by_detail[..FINE_LEVELS].fill(Vec::new());
    }

    /// The ground of the superchunk whose top left cell is `top_left`,
    /// in the world `world` the host runs, drawn between `height_levels`: the
    /// heights `given_heights` taken as they are, those about them asked of
    /// the host -- which, running another world by now, is not
    /// waited for: the ground is flat at the ocean's level, for the
    /// one frame of a world gone.
    pub fn ask_of_host(terrain: &dyn AsksTerrain, world: u64, height_levels: DrawnHeightRange, top_left: (u32, u32), given_heights: &GivenHeights) -> Self {
        Self::of_heights(&heights(terrain, world, top_left, given_heights).unwrap_or_else(|| vec![height_levels.ocean; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS]), height_levels)
    }

    /// The ground of a superchunk and the cells about it whose heights
    /// are `heights`, [`WORKED_SIDE_CELLS`] a side, row by row, drawn between `height_levels`.
    fn of_heights(heights: &[Height], height_levels: DrawnHeightRange) -> Self {
        let (ocean_and_highest, deepest_ocean) = ((height_levels.ocean, height_levels.highest_land), height_levels.ocean.saturating_sub(height_levels.lowest_height) as f32);
        let smoothed_heights = box_smoothed(&box_smoothed(&heights.iter().map(|&height| height as f32).collect::<Vec<_>>()));
        let (lines, shadowed_cells) = shadow_lines(heights);
        let to_coast = coast_distances(&heights.iter().map(|&height| height < height_levels.ocean).collect::<Vec<_>>());
        let worked_index = |x: usize, y: usize| (y + CELLS_KEPT_BEFORE) * WORKED_SIDE_CELLS + x + CELLS_KEPT_BEFORE;
        // Each cell's light; and for the levels, what its colour is multiplied by, its height and how far the coast is.
        let mut cell_lights = vec![0; SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE];
        let mut ground_at_detail = GroundAtDetail { side_pixels: SUPERCHUNK_CELLS_A_SIDE, colour_factors: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE), untinted_factors: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE), pale_shares: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE), heights: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE), ocean_shares: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE), to_coast: Vec::with_capacity(SUPERCHUNK_CELLS_A_SIDE * SUPERCHUNK_CELLS_A_SIDE) };
        for y in 0..SUPERCHUNK_CELLS_A_SIDE {
            for x in 0..SUPERCHUNK_CELLS_A_SIDE {
                let here_index = worked_index(x, y);
                let (slope_across, slope_down) = ((smoothed_heights[here_index + 1] - smoothed_heights[here_index - 1]) / 2.0, (smoothed_heights[here_index + WORKED_SIDE_CELLS] - smoothed_heights[here_index - WORKED_SIDE_CELLS]) / 2.0);
                let slope_lit = banded_light(slope_light(slope_across, slope_down), LIGHT_BAND);
                let is_under_ocean = heights[here_index] < height_levels.ocean;
                // A shadow falls on the water, not on the ground under it.
                let shadow_falls = if is_under_ocean { lines[here_index] > height_levels.ocean as f32 + 0.01 } else { shadowed_cells[here_index] };
                cell_lights[y * SUPERCHUNK_CELLS_A_SIDE + x] = (slope_lit * LIT_ONE).round() as u8 | if shadow_falls { SHADOWED_BIT } else { 0 };
                // Water's light is its depth's; the land's its slope's, tinted by its height.
                let (colour_factor, pale_share) = if is_under_ocean { (water_light((height_levels.ocean - heights[here_index]) as f32, deepest_ocean), 0.0) } else { (height_tint(height_share(heights[here_index], ocean_and_highest)).0.map(|tint_channel| tint_channel * slope_lit), height_tint(height_share(heights[here_index], ocean_and_highest)).1) };
                let shadow_factor = if shadow_falls { CAST_SHADOW_FACTORS } else { [1.0; 3] };
                ground_at_detail.colour_factors.push(std::array::from_fn(|colour_channel| colour_factor[colour_channel] * shadow_factor[colour_channel]));
                ground_at_detail.untinted_factors.push(shadow_factor.map(|shadow_channel| shadow_channel * slope_lit));
                ground_at_detail.pale_shares.push(pale_share);
                ground_at_detail.heights.push(heights[here_index] as f32);
                ground_at_detail.ocean_shares.push(is_under_ocean as u8 as f32);
                ground_at_detail.to_coast.push(to_coast[here_index]);
            }
        }
        let mut shades_by_detail = Vec::with_capacity(COARSEST_DETAIL + 1);
        for detail_shift in 0..=COARSEST_DETAIL {
            shades_by_detail.push(ground_at_detail.drawn_at_detail(detail_shift));
            ground_at_detail = ground_at_detail.twice_as_coarse();
        }
        // What is kept: the superchunk's cells and the margin all round.
        let kept_indices = || (0..KEPT_SIDE_CELLS * KEPT_SIDE_CELLS).map(|kept_index| (kept_index / KEPT_SIDE_CELLS + CELLS_KEPT_BEFORE - KEPT_MARGIN_CELLS) * WORKED_SIDE_CELLS + kept_index % KEPT_SIDE_CELLS + CELLS_KEPT_BEFORE - KEPT_MARGIN_CELLS);
        let fine_ground = FineGround { heights: kept_indices().map(|height_index| heights[height_index]).collect(), lines: kept_indices().map(|line_index| lines[line_index]).collect(), cell_lights, ocean_and_highest, deepest_ocean };
        Self { shades_by_detail, fine_ground: Some(fine_ground), last_used_frame: 0 }
    }
}

/// Heights already worked out: a superchunk's height words
/// ([`height_in_frame`]) by its top left cell.
pub type GivenHeights<'a> = HashMap<(u32, u32), &'a [u64]>;

/// The heights about the superchunk whose top left cell is `top_left`
/// in the world `world`: [`WORKED_SIDE_CELLS`] a side, row by row. Those of a
/// superchunk `given_heights` are read; the others are asked of the host,
/// which is told to pass the given ones by -- none if it runs another
/// world by now.
fn heights(terrain: &dyn AsksTerrain, world: u64, top_left: (u32, u32), given_heights: &GivenHeights) -> Option<Vec<Height>> {
    let (worked_left_column, worked_top_row) = (top_left.0.wrapping_sub(CELLS_KEPT_BEFORE as u32), top_left.1.wrapping_sub(CELLS_KEPT_BEFORE as u32));
    let mut heights = terrain.heights(HeightsAsk { world, first_cell_cartesian: (worked_left_column, worked_top_row), size_in_cells: (WORKED_SIDE_CELLS as u32, WORKED_SIDE_CELLS as u32), skipped_corners_cartesian: given_heights.keys().copied().collect() })?;
    for (row_number, heights) in heights.chunks_mut(WORKED_SIDE_CELLS).enumerate() {
        let (y, mut cells_across) = (worked_top_row.wrapping_add(row_number as u32), 0);
        while cells_across < WORKED_SIDE_CELLS {
            // The row's run within one superchunk.
            let x = worked_left_column.wrapping_add(cells_across as u32);
            let (in_x, in_y) = (x % SUPERCHUNK_CELLS_A_SIDE as u32, y % SUPERCHUNK_CELLS_A_SIDE as u32);
            let superchunk_run = (SUPERCHUNK_CELLS_A_SIDE - in_x as usize).min(WORKED_SIDE_CELLS - cells_across);
            if let Some(words) = given_heights.get(&(x - in_x, y - in_y)) {
                heights[cells_across..cells_across + superchunk_run].iter_mut().zip(0u32..).for_each(|(height, cells_along)| *height = height_in_frame(words, place_from_cartesian(in_x + cells_along, in_y)));
            }
            cells_across += superchunk_run;
        }
    }
    Some(heights)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two superchunks side by side, and one below: the shadow lines
    /// over the cells they both keep are the same from either; and the
    /// heights a frame brings are the ones the host answers when asked.
    #[test]
    fn shadows_are_the_same_from_both_sides_of_an_edge() {
        use protocol::pipe::frame::FrameAsk;
        let world_start = server::WorldStart { seed: server::seed_with_land(crate::renderer::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT), world_generation: server::WorldGeneration { sheep_per_superchunk: 1, ..server::WorldGeneration::DEFAULT }, ..server::WorldStart::default() };
        let (host, frame_receiver) = server::host::Host::start_on_thread();
        assert!(host.set_paused(true) && host.make_world(world_start) && host.ask_frame(FrameAsk { viewport: None, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 0, seen_near: None }));
        let first_frame = frame_receiver.recv_timeout(std::time::Duration::from_secs(1800)).expect("a frame of the world made");
        let terrain = host.terrain();
        let origin_top_left = coordinates::WORLD_MIDDLE.top_left().cartesian();
        let (origin_left, origin_top, side_cells) = (origin_top_left.x, origin_top_left.y, SUPERCHUNK_CELLS_A_SIDE as u32);
        let fine_ground_at = |top_left: (u32, u32), given_heights: &GivenHeights| SuperchunkGround::ask_of_host(&terrain, first_frame.world, first_frame.drawn_heights, top_left, given_heights).fine_ground.expect("made fine");
        let (origin_ground, beside_ground, below_ground) = (fine_ground_at((origin_left, origin_top), &GivenHeights::new()), fine_ground_at((origin_left + side_cells, origin_top), &GivenHeights::new()), fine_ground_at((origin_left, origin_top + side_cells), &GivenHeights::new()));
        let mut differing_lines = 0;
        for cells_along in 0..SUPERCHUNK_CELLS_A_SIDE as isize {
            for cells_off in 0..KEPT_MARGIN_CELLS as isize {
                differing_lines += (origin_ground.shadow_line(SUPERCHUNK_CELLS_A_SIDE as isize + cells_off, cells_along) != beside_ground.shadow_line(cells_off, cells_along)) as usize;
                differing_lines += (origin_ground.shadow_line(cells_along, SUPERCHUNK_CELLS_A_SIDE as isize + cells_off) != below_ground.shadow_line(cells_along, cells_off)) as usize;
            }
        }
        assert_eq!(differing_lines, 0, "shadow lines unlike over an edge");
        let world = server::start_world(world_start);
        let image = world.chunk_storage().image(coordinates::WORLD_MIDDLE).expect("the origin's image");
        let brought_ground = fine_ground_at((origin_left, origin_top), &[((origin_left, origin_top), image.height_words())].into_iter().collect());
        let cells = || (0..SUPERCHUNK_CELLS_A_SIDE as isize).flat_map(|y| (0..SUPERCHUNK_CELLS_A_SIDE as isize).map(move |x| (x, y)));
        assert!(cells().all(|(x, y)| brought_ground.height(x, y) == origin_ground.height(x, y) && brought_ground.shadow_line(x, y) == origin_ground.shadow_line(x, y)), "heights brought unlike those asked for");
        // Asked of a world the host does not run: no answer, and a ground flat at the ocean's level.
        assert!(heights(&terrain, first_frame.world + 1, (origin_left, origin_top), &GivenHeights::new()).is_none());
    }
}
