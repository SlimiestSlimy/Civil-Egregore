//! The world from near, a cell several pixels: one picture of the
//! viewport's cells, height drawn at its edges -- a line at a step, a
//! band under a wall (`docs/renderer.md`, "Height, from straight
//! above" and "Corners").

use crate::renderer::ground::relief::{laid_over, tint_on_sand, water_light, FOAM_COLOUR, FOAM_MOST, PALE_COLOUR, SAND_COLOUR, SAND_MOST};
use crate::renderer::ground::{shadow_drop, FineGround, SuperchunkGround, KEPT_MARGIN_CELLS, SUPERCHUNK_CELLS_A_SIDE};
use crate::renderer::paint::{depth_at, each_cloud, in_rain, stage_at, streak_of_rain_on, tree_colour, under_cloud, under_water};
use basegame::base_types::plane_words::DEEPEST_CLOUD;
use basegame::base_types::{GRASS, OLDEST_TREE_STAGE, TREE, WET};
use protocol::pipe::frame::{bit_set, Cells, NearRectangle, WORD_BITS};
use basegame::base_types::DEEP_WATER_DEPTH;
use utilities::tuning::{Tuning, RELIEF_STRENGTH, CAST_SHADOW, STEP_DARK, STEP_LIGHT, GROUND_TEXTURE, WALL_FADE, WALL_LENGTH, WALL_LIT, WALL_SHADE};
use utilities::coordinates::place_from_cartesian;
use std::collections::HashMap;
use utilities::hash::mix_word;
use crate::renderer::paint::{DIRT_BROWN, GRASS_GREEN, SHEEP_WHITE};

/// A cell's side in eighths: what edges are measured in, whatever the
/// pixels a cell.
const EIGHTHS_A_CELL: f32 = 8.0;

/// The eight cells about a cell: across and down.
const AROUND: [(isize, isize); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// How much lighter or darker a pixel's ground is than its cell's
/// colour, by lot: most as it is, some a little off.
const PIXEL_TONES: [f32; 16] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.08, 1.08, 1.08, 1.08, 0.92, 0.92, 0.92, 1.2];

/// A sheep on a cell eight pixels a side, from above.
const SHEEP_SHAPE: [&str; 8] = ["........", "........", "..####..", ".######.", ".######.", "..####..", "..#..#..", "........"];

/// The viewport's cells from near, painted.
pub struct PaintedNear {
    /// What was asked for.
    pub near_asked: NearRectangle,
    /// Its pixels, row by row: red, green, blue, opacity.
    pub rgba_bytes: Vec<u8>,
}

/// An edge of a cell: a neighbour of another height.
#[derive(Clone, Copy, Default)]
struct CellEdge {
    /// Towards the neighbour: across and down.
    towards_neighbour: (isize, isize),
    /// How much higher the neighbour is: for a wall, than the cell at
    /// its foot.
    neighbour_rise: i32,
    /// How far off the wall is, in eighths: the cells between.
    wall_away_eighths: f32,
}

/// How long the band of a wall rising `wall_rise` is, in eighths of a
/// cell: no longer than is looked for it.
fn wall_band_eighths(wall_rise: i32, tuning: &Tuning) -> f32 {
    (4.0 + wall_rise as f32 * tuning[WALL_LENGTH]).clamp(5.0, EIGHTHS_A_CELL * KEPT_MARGIN_CELLS as f32)
}

impl CellEdge {
    /// What the edge does to a pixel `from_eighths` eighths in from it and
    /// `span_eighths` wide: how dark it makes it and how light, 1 neither --
    /// by as much as `tuning` says.
    fn pixel_shading(self, from_eighths: f32, span_eighths: f32, tuning: &Tuning) -> (f32, f32) {
        // The share of the pixel within a band of the edge.
        let share_within = |band_nearest: f32, band_farthest: f32| ((from_eighths + span_eighths).min(band_farthest) - from_eighths.max(band_nearest)).max(0.0) / span_eighths;
        // The sun is up and to the left: a neighbour down or to the right shows it its face, one up or to the left its back.
        let sun_facing = self.towards_neighbour.0 + self.towards_neighbour.1;
        match self.neighbour_rise {
            2.. => {
                // A wall: a band on its lower cell, darkest at its foot -- by one number if the wall faces away from the sun, by another if the sun is on it, and between the two at a corner that faces neither way.
                // The higher the wall the longer the band, over the cells before it, and no darker.
                let (width, from_wall_foot) = (wall_band_eighths(self.neighbour_rise, tuning), from_eighths + self.wall_away_eighths);
                let foot_darkness = match sun_facing {
                    ..0 => tuning[WALL_SHADE],
                    0 => (tuning[WALL_SHADE] + tuning[WALL_LIT]) / 2.0,
                    _ => tuning[WALL_LIT],
                };
                (1.0 - foot_darkness * (1.0 - tuning[WALL_FADE] * from_wall_foot / width).max(0.0) * share_within(0.0, width), 1.0)
            }
            // The border of the higher cell: light where it looks to the sun, over a lower cell up or to the left; dark where it looks away.
            ..=-1 if sun_facing < 0 => (1.0, 1.0 + tuning[STEP_LIGHT] * share_within(0.0, 1.0)),
            ..=-1 if sun_facing > 0 => (1.0 - tuning[STEP_DARK] * share_within(0.0, 1.0), 1.0),
            _ => (1.0, 1.0),
        }
    }
}

/// The cells `near_asked` asks for as one picture, from `cells` -- the
/// superchunks they are in -- and their `kept_grounds`, shaded as `tuning`
/// says. A cold superchunk, and one past the world shown, is black.
pub fn paint_near(cells: &[Cells], kept_grounds: &HashMap<(u32, u32), SuperchunkGround>, near_asked: NearRectangle, tuning: &Tuning) -> PaintedNear {
    let pixels_a_cell = near_asked.pixels_a_cell as usize;
    let (first_cell_cartesian, size_cells) = ((near_asked.first_cell_cartesian.0 as usize, near_asked.first_cell_cartesian.1 as usize), (near_asked.size_in_cells.0 as usize, near_asked.size_in_cells.1 as usize));
    let width = size_cells.0 * pixels_a_cell;
    let mut rgba_pixels = vec![[0, 0, 0, u8::MAX]; width * size_cells.1 * pixels_a_cell];
    for cells in cells {
        let Some(fine_ground) = kept_grounds.get(&cells.top_left).and_then(|kept_ground| kept_ground.fine_ground.as_ref()) else {
            continue;
        };
        let (superchunk_left_column, superchunk_top_row) = (cells.superchunk_cartesian.0 as usize * SUPERCHUNK_CELLS_A_SIDE, cells.superchunk_cartesian.1 as usize * SUPERCHUNK_CELLS_A_SIDE);
        // The superchunk's cells in the picture, from its own top left.
        let picture_columns = first_cell_cartesian.0.max(superchunk_left_column)..(first_cell_cartesian.0 + size_cells.0).min(superchunk_left_column + SUPERCHUNK_CELLS_A_SIDE);
        let picture_rows = first_cell_cartesian.1.max(superchunk_top_row)..(first_cell_cartesian.1 + size_cells.1).min(superchunk_top_row + SUPERCHUNK_CELLS_A_SIDE);
        let (grass, wet, trees) = (cells.plane(GRASS), cells.plane(WET), cells.plane(TREE));
        for y in picture_rows.clone() {
            for x in picture_columns.clone() {
                let (own_x, own_y) = (x - superchunk_left_column, y - superchunk_top_row);
                let place = place_from_cartesian(own_x as u32, own_y as u32);
                let is_grass = bit_set(grass, place);
                let (word, bit_in_word) = (place / WORD_BITS, (place % WORD_BITS) as u32);
                let water = bit_set(wet, place).then(|| depth_at(cells, word, bit_in_word));
                let cell = Cell { fine_ground, tuning, own_cartesian: (own_x, own_y), world: (cells.top_left.0 as u64 + own_x as u64, cells.top_left.1 as u64 + own_y as u64), cover_colour: if is_grass { GRASS_GREEN } else { DIRT_BROWN }, water };
                let corner_pixel = ((x - first_cell_cartesian.0) * pixels_a_cell, (y - first_cell_cartesian.1) * pixels_a_cell);
                cell.paint_cell(&mut rgba_pixels, width, corner_pixel, pixels_a_cell);
                if bit_set(trees, place) {
                    tree(&mut rgba_pixels, width, corner_pixel, pixels_a_cell, stage_at(cells, word, bit_in_word));
                }
            }
        }
        for &(x, y) in &cells.entity_cells_cartesian {
            let (x, y) = (superchunk_left_column + x as usize, superchunk_top_row + y as usize);
            if picture_columns.contains(&x) && picture_rows.contains(&y) {
                paint_sheep(&mut rgba_pixels, width, ((x - first_cell_cartesian.0) * pixels_a_cell, (y - first_cell_cartesian.1) * pixels_a_cell), pixels_a_cell);
            }
        }
        // The clouds' shadows over all of it: each tile's, over those of its cells that are in the picture.
        each_cloud(cells, |(tile_left, tile_top), tile_side, cloud, is_raining| {
            let (tile_across, tile_down) = (picture_columns.start.max(superchunk_left_column + tile_left)..picture_columns.end.min(superchunk_left_column + tile_left + tile_side), picture_rows.start.max(superchunk_top_row + tile_top)..picture_rows.end.min(superchunk_top_row + tile_top + tile_side));
            let pixel_rows = (tile_down.start - first_cell_cartesian.1) * pixels_a_cell..(tile_down.end.max(tile_down.start) - first_cell_cartesian.1) * pixels_a_cell;
            let pixel_columns = (tile_across.start - first_cell_cartesian.0) * pixels_a_cell..(tile_across.end.max(tile_across.start) - first_cell_cartesian.0) * pixels_a_cell;
            for (pixel_row, pixel_column) in pixel_rows.flat_map(|flat_row| pixel_columns.clone().map(move |flat_column| (flat_row, flat_column))) {
                let pixel_number = pixel_row * width + pixel_column;
                let clouded_colour = under_cloud([rgba_pixels[pixel_number][0], rgba_pixels[pixel_number][1], rgba_pixels[pixel_number][2]], cloud as usize, DEEPEST_CLOUD as usize);
                // Rain under a cloud deep enough: a streak on some of its cells, each cell's pixels alike.
                let rained_colour = if is_raining && streak_of_rain_on(first_cell_cartesian.0 + pixel_column / pixels_a_cell, first_cell_cartesian.1 + pixel_row / pixels_a_cell) { in_rain(clouded_colour, 1, 1) } else { clouded_colour };
                rgba_pixels[pixel_number] = [rained_colour[0], rained_colour[1], rained_colour[2], u8::MAX];
            }
        });
    }
    PaintedNear { near_asked, rgba_bytes: rgba_pixels.into_flattened() }
}

/// A cell to paint.
struct Cell<'a> {
    /// Its superchunk's ground.
    fine_ground: &'a FineGround,
    /// What its shading is tuned by.
    tuning: &'a Tuning,
    /// Where it is, from its superchunk's top left.
    own_cartesian: (usize, usize),
    /// Where it is in the world.
    world: (u64, u64),
    /// Its colour: dirt's or grass's.
    cover_colour: [u8; 3],
    /// How deep the water over it is, if it is wet.
    water: Option<u32>,
}

impl Cell<'_> {
    /// Paints the cell `pixels_a_cell` a side into `rgba_pixels`, `width` a
    /// row, its top left pixel at `corner_pixel`.
    fn paint_cell(&self, rgba_pixels: &mut [[u8; 4]], width: usize, corner_pixel: (usize, usize), pixels_a_cell: usize) {
        let (x, y) = (self.own_cartesian.0 as isize, self.own_cartesian.1 as isize);
        let own_height = self.fine_ground.height(x, y);
        let (mut cell_edges, mut count) = ([CellEdge::default(); 12], 0);
        let tuning = self.tuning;
        let rise_towards = |towards_cell: (isize, isize)| self.fine_ground.height(x + towards_cell.0, y + towards_cell.1) as i32 - own_height as i32;
        // The wall met first going `towards_cell`, over ground that does not drop as a wall would: how many cells off it stands and how much it rises, if its band reaches this cell.
        let wall_towards = |towards_wall: (isize, isize)| {
            let step = |cells_off: isize| rise_towards((towards_wall.0 * cells_off, towards_wall.1 * cells_off)) - rise_towards((towards_wall.0 * (cells_off - 1), towards_wall.1 * (cells_off - 1)));
            let met_wall = (1..=KEPT_MARGIN_CELLS as isize).find_map(|search_off| match step(search_off) {
                2.. => Some(Some(search_off)),
                ..=-2 => Some(None),
                _ => None,
            });
            let wall_off = met_wall.flatten()?;
            (((wall_off - 1) as f32) * EIGHTHS_A_CELL < wall_band_eighths(step(wall_off), tuning)).then_some((wall_off, step(wall_off)))
        };
        for towards_neighbour in AROUND {
            let neighbour_rise = rise_towards(towards_neighbour);
            let is_corner = towards_neighbour.0 != 0 && towards_neighbour.1 != 0;
            if neighbour_rise <= -1 && !is_corner {
                cell_edges[count] = CellEdge { towards_neighbour, neighbour_rise, wall_away_eighths: 0.0 };
                count += 1;
            }
            let Some((wall_cells_off, wall_rise)) = wall_towards(towards_neighbour) else { continue };
            // A corner alone says nothing: one counts only where the cells either side of it are no higher than this one, and no wall is as near straight up or across -- where it joins their two bands.
            let straight_nearer = |straight_towards: (isize, isize)| rise_towards(straight_towards) > 0 || wall_towards(straight_towards).is_some_and(|(straight_off, _)| straight_off <= wall_cells_off);
            if !is_corner || !(straight_nearer((towards_neighbour.0, 0)) || straight_nearer((0, towards_neighbour.1))) {
                cell_edges[count] = CellEdge { towards_neighbour, neighbour_rise: wall_rise, wall_away_eighths: (wall_cells_off - 1) as f32 * EIGHTHS_A_CELL };
                count += 1;
            }
        }
        // Under deep water nothing of the ground shows, its edges neither.
        if self.water.is_some_and(|water_depth| water_depth >= DEEP_WATER_DEPTH) {
            count = 0;
        }
        // Its colour under its water; sand on the land beside the ocean, foam on the ocean beside the land; and what its height tints it, or its depth.
        let ocean_within = |reach: isize, ocean: bool| (-reach..=reach).flat_map(|reach_down| (-reach..=reach).map(move |reach_across| (reach_across, reach_down))).any(|(look_across, look_down)| self.fine_ground.under_ocean(x + look_across, y + look_down).is_some() == ocean);
        let watered_colour = self.water.map_or(self.cover_colour, |cell_water_depth| under_water(self.cover_colour, cell_water_depth));
        let (laid_colour, tint_light) = match self.fine_ground.under_ocean(x, y) {
            Some(ocean_depth) => (if ocean_within(1, false) { laid_over(watered_colour, FOAM_COLOUR, FOAM_MOST) } else { watered_colour }, water_light(ocean_depth as f32, self.fine_ground.deepest_ocean_depth())),
            None => {
                let (land_tint, pale_share) = self.fine_ground.cell_tint(x, y);
                let sand_share = if ocean_within(1, true) { SAND_MOST } else if ocean_within(2, true) { SAND_MOST / 2.0 } else { 0.0 };
                (if sand_share >= pale_share { laid_over(watered_colour, SAND_COLOUR, sand_share) } else { laid_over(watered_colour, PALE_COLOUR, pale_share) }, tint_on_sand(land_tint, sand_share))
            }
        };
        // Water is smooth: its pixels do not differ by lot as the ground's do.
        let texture_strength = if self.water.is_some() { 0.0 } else { tuning[GROUND_TEXTURE] };
        // The shadow lines that reach the cell: down the diagonal, and from above and from the left.
        let (drop, surface_over) = (shadow_drop(), self.fine_ground.shadow_surface(x, y) as f32 + 0.01);
        let (shadow_diagonal, shadow_above, shadow_beside) = (self.fine_ground.shadow_line(x - 1, y - 1) - surface_over, self.fine_ground.shadow_line(x, y - 1) - surface_over, self.fine_ground.shadow_line(x - 1, y) - surface_over);
        let may_be_shadowed = shadow_diagonal.max(shadow_above).max(shadow_beside) > 0.0;
        let relief_light = 1.0 + tuning[RELIEF_STRENGTH] * (self.fine_ground.cell_light(self.own_cartesian.0, self.own_cartesian.1) - 1.0);
        // A cast shadow: darker, and bluer.
        let shadow_tint = [1.0 - tuning[CAST_SHADOW], 1.0 - 0.885 * tuning[CAST_SHADOW], 1.0 - 0.46 * tuning[CAST_SHADOW]];
        let pixel_span = EIGHTHS_A_CELL / pixels_a_cell as f32;
        for pixel_down in 0..pixels_a_cell {
            for pixel_across in 0..pixels_a_cell {
                // How far in the pixel is from the cell's left, right, top and bottom, in eighths.
                let (from_left, from_top) = (pixel_across as f32 * pixel_span, pixel_down as f32 * pixel_span);
                let (from_right, from_bottom) = (EIGHTHS_A_CELL - pixel_span - from_left, EIGHTHS_A_CELL - pixel_span - from_top);
                let (mut darkest_shading, mut brightest_shading) = (1.0f32, 1.0f32);
                for cell_edge in &cell_edges[..count] {
                    let from_edge = match cell_edge.towards_neighbour {
                        (-1, 0) => from_left,
                        (1, 0) => from_right,
                        (0, -1) => from_top,
                        (0, 1) => from_bottom,
                        // A corner's: within its square only.
                        (-1, -1) => from_left.max(from_top),
                        (1, -1) => from_right.max(from_top),
                        (-1, 1) => from_left.max(from_bottom),
                        _ => from_right.max(from_bottom),
                    };
                    let (edge_darkens, edge_brightens) = cell_edge.pixel_shading(from_edge, pixel_span, tuning);
                    (darkest_shading, brightest_shading) = (darkest_shading.min(edge_darkens), brightest_shading.max(edge_brightens));
                }
                let in_shadow = may_be_shadowed && {
                    // The pixel's middle, in cells from the cell's top left: the shadow line has dropped by how far it has come.
                    let (in_x, in_y) = ((pixel_across as f32 + 0.5) / pixels_a_cell as f32, (pixel_down as f32 + 0.5) / pixels_a_cell as f32);
                    shadow_diagonal > drop * in_x.max(in_y) || if in_x > in_y { shadow_above > drop * in_y } else { shadow_beside > drop * in_x }
                };
                // One edge's doing a pixel, a little blue as the shadows are; and under a cast shadow an edge still shows, darker than it.
                let pixel_shade = match (darkest_shading < 1.0, in_shadow) {
                    (true, true) => [darkest_shading * shadow_tint[0], darkest_shading * shadow_tint[1], (darkest_shading * 1.08).min(1.0) * shadow_tint[2]],
                    (true, false) => [darkest_shading, darkest_shading, (darkest_shading * 1.08).min(1.0)],
                    (false, true) => shadow_tint.map(|shadow_channel| shadow_channel * brightest_shading),
                    (false, false) => [brightest_shading; 3],
                };
                let (pixel_x, pixel_y) = (self.world.0 * pixels_a_cell as u64 + pixel_across as u64, self.world.1 * pixels_a_cell as u64 + pixel_down as u64);
                // Each mixed in whole: a world pixel's place takes more than 32 bits.
                let pixel_tone = (1.0 + texture_strength * (PIXEL_TONES[(mix_word(mix_word(pixel_x) ^ pixel_y) >> 60) as usize] - 1.0)) * relief_light;
                let shaded_colour: [u8; 3] = std::array::from_fn(|colour_channel| (laid_colour[colour_channel] as f32 * pixel_tone * tint_light[colour_channel] * pixel_shade[colour_channel]).round().min(255.0) as u8);
                rgba_pixels[(corner_pixel.1 + pixel_down) * width + corner_pixel.0 + pixel_across] = [shaded_colour[0], shaded_colour[1], shaded_colour[2], u8::MAX];
            }
        }
    }
}

/// A tree `stage` old on the cell whose top left pixel is at `corner_pixel`:
/// a square in the cell's middle, larger and darker the older it is,
/// from three eighths of the cell to the whole of it.
fn tree(rgba_pixels: &mut [[u8; 4]], width: usize, corner_pixel: (usize, usize), pixels_a_cell: usize, stage: u32) {
    let crown_colour = tree_colour(stage);
    let crown_side = (pixels_a_cell * (3 + 5 * stage as usize / OLDEST_TREE_STAGE as usize) / 8).max(1);
    let crown_inset = (pixels_a_cell - crown_side) / 2;
    for pixel_down in crown_inset..crown_inset + crown_side {
        for pixel_across in crown_inset..crown_inset + crown_side {
            // Its lower right edge a little darker: a crown, lit from the top left.
            let shade_quarters = if crown_side > 2 && (pixel_down + 1 == crown_inset + crown_side || pixel_across + 1 == crown_inset + crown_side) { 3 } else { 4 };
            let shaded_pixel = crown_colour.map(|colour_channel| (colour_channel as usize * shade_quarters / 4) as u8);
            rgba_pixels[(corner_pixel.1 + pixel_down) * width + corner_pixel.0 + pixel_across] = [shaded_pixel[0], shaded_pixel[1], shaded_pixel[2], u8::MAX];
        }
    }
}

/// A sheep on the cell whose top left pixel is at `corner_pixel`: its shape,
/// drawn as coarsely as the cell is.
fn paint_sheep(rgba_pixels: &mut [[u8; 4]], width: usize, corner_pixel: (usize, usize), pixels_a_cell: usize) {
    let shape_per_pixel = SHEEP_SHAPE.len() / pixels_a_cell;
    for pixel_down in 0..pixels_a_cell {
        for pixel_across in 0..pixels_a_cell {
            // White where a quarter of what is under the pixel is the sheep's.
            let sheep_points = (0..shape_per_pixel * shape_per_pixel).filter(|point_number| SHEEP_SHAPE[pixel_down * shape_per_pixel + point_number / shape_per_pixel].as_bytes()[pixel_across * shape_per_pixel + point_number % shape_per_pixel] == b'#').count();
            if 4 * sheep_points >= shape_per_pixel * shape_per_pixel {
                rgba_pixels[(corner_pixel.1 + pixel_down) * width + corner_pixel.0 + pixel_across] = [SHEEP_WHITE[0], SHEEP_WHITE[1], SHEEP_WHITE[2], u8::MAX];
            }
        }
    }
}
