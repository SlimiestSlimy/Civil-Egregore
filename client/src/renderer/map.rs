//! The map: the world as generated, shown in place of its cells in
//! map mode, its cells asked of the host and drawn on a thread of its
//! own (`docs/renderer.md`, "The map").

use crate::renderer::frames::{picture_of, LaidPictureParts, DIRT};
use crate::renderer::link::LastFrameSeen;
use crate::renderer::ground::relief::{laid_over, slope_light, height_tint, tint_on_sand, water_light, FOAM_COLOUR, FOAM_MOST, PALE_COLOUR, SAND_COLOUR, SAND_MOST};
use crate::renderer::paint::{tree_colour, DIRT_BROWN, GRASS_GREEN, WATER};
use crate::renderer::view::origin;
use bevy::prelude::*;
use crate::gui::CapturedInput;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use protocol::pipe::terrain::{AsksTerrain, CellCover, MapAsk};

/// Pixels of the map past each of the view's edges: what a moving view
/// shows before the next map comes.
const MAP_MARGIN_PIXELS: i64 = 64;
/// How much of its light a pixel on a line of the mesh keeps.
const BORDER_LIGHT: f32 = 0.25;

/// A map asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WantedMap {
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first_cell_cartesian: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size_pixels: (u32, u32),
    /// The world it is of, as the host numbers those it runs.
    pub world: u64,
    /// Whether the mesh's lines are drawn over it.
    pub mesh_lines_drawn: bool,
}

/// A map drawn.
pub struct DrawnMap {
    /// What was asked for.
    pub wanted_map: WantedMap,
    /// Its pixels, red, green, blue and opacity, row by row.
    pub rgba_bytes: Vec<u8>,
}

/// Starts the map's thread: where to ask for a map, and where it
/// comes back, its cells asked of the host through `terrain`. Of
/// several asked for while one is drawn, only the last is drawn next;
/// one of a world the host runs no more is not drawn.
pub fn start_map_thread(terrain: impl AsksTerrain + 'static) -> (Sender<WantedMap>, Receiver<DrawnMap>) {
    let (wanted_sender, wanted_receiver) = channel::<WantedMap>();
    let (drawn_sender, maps) = channel();
    thread::Builder::new()
        .name("map".to_string())
        .spawn(move || {
            while let Ok(first_wanted) = wanted_receiver.recv() {
                let wanted_map = wanted_receiver.try_iter().last().unwrap_or(first_wanted);
                if let Some(rgba_bytes) = draw_map(&terrain, &wanted_map)
                    && drawn_sender.send(DrawnMap { wanted_map, rgba_bytes }).is_err()
                {
                    return;
                }
            }
        })
        .expect("a thread for the map");
    (wanted_sender, maps)
}

/// The pixels of `wanted_map`, its cells asked of the host through
/// `terrain`: none if it runs another world by now.
pub fn draw_map(terrain: &dyn AsksTerrain, wanted_map: &WantedMap) -> Option<Vec<u8>> {
    let terrain_answer = terrain.map(MapAsk { world: wanted_map.world, first_cell_cartesian: wanted_map.first_cell_cartesian, step: wanted_map.step, size_in_pixels: wanted_map.size_pixels, mesh_lines_asked: wanted_map.mesh_lines_drawn })?;
    let (width, height_levels) = (wanted_map.size_pixels.0 as usize, terrain_answer.drawn_heights);
    let mut rgba_bytes = vec![0u8; terrain_answer.cells.len() * 4];
    for (cells, row_bytes) in terrain_answer.cells.chunks(width.max(1)).zip(rgba_bytes.chunks_mut(width.max(1) * 4)) {
        // The height of the pixel to the left of each.
        let mut left_height = None;
        for (cell, pixel_bytes) in cells.iter().zip(row_bytes.chunks_mut(4)) {
            let Some(cell) = cell else {
                pixel_bytes.copy_from_slice(&[0, 0, 0, u8::MAX]);
                left_height = None;
                continue;
            };
            // The pixels up and to the left of it, the sun's side: what its slope is told by, and whether the coast passes between.
            let (cell_height, height_above, height_beside) = (cell.height, cell.height_above, left_height.unwrap_or(cell.height));
            let on_coast = |ocean: bool| (height_above < height_levels.ocean) == ocean || (height_beside < height_levels.ocean) == ocean;
            let cover_colour = match cell.cell_cover {
                CellCover::Ocean => WATER,
                CellCover::Tree => tree_colour(8),
                CellCover::Grass => GRASS_GREEN,
                CellCover::Dirt => DIRT_BROWN,
            };
            let (laid_colour, channel_light) = if cell.cell_cover == CellCover::Ocean {
                (if on_coast(false) { laid_over(cover_colour, FOAM_COLOUR, FOAM_MOST) } else { cover_colour }, water_light((height_levels.ocean - cell_height) as f32, height_levels.ocean.saturating_sub(height_levels.lowest_height) as f32))
            } else {
                let (land_tint, pale_share) = height_tint(cell_height.saturating_sub(height_levels.ocean) as f32 / height_levels.highest_land.saturating_sub(height_levels.ocean).max(1) as f32);
                let (sand_or_pale_colour, final_tint) = if on_coast(true) { (laid_over(cover_colour, SAND_COLOUR, SAND_MOST), tint_on_sand(land_tint, SAND_MOST)) } else { (laid_over(cover_colour, PALE_COLOUR, pale_share), land_tint) };
                let slope_lights = slope_light((cell_height as f32 - height_beside as f32) / wanted_map.step as f32, (cell_height as f32 - height_above as f32) / wanted_map.step as f32);
                (sand_or_pale_colour, final_tint.map(|tint_channel| tint_channel * slope_lights))
            };
            left_height = Some(cell_height);
            // A line of the mesh, where the pixel is no farther from it than it is across.
            let line_light = if cell.on_a_mesh_line { channel_light.map(|light_channel| light_channel * BORDER_LIGHT) } else { channel_light };
            let lit_rgb: [u8; 3] = std::array::from_fn(|colour_channel| (laid_colour[colour_channel] as f32 * line_light[colour_channel]).min(255.0) as u8);
            pixel_bytes.copy_from_slice(&[lit_rgb[0], lit_rgb[1], lit_rgb[2], u8::MAX]);
        }
    }
    Some(rgba_bytes)
}

/// The map, as the window holds it: where to ask for one, where it
/// comes back, what was last asked for, and whether it is shown.
#[derive(Resource)]
pub struct MapLink {
    /// Where the maps wanted go.
    wanted_sender: Sender<WantedMap>,
    /// Where they come back, drawn.
    maps: Mutex<Receiver<DrawnMap>>,
    /// The last asked for.
    last_asked: Option<WantedMap>,
    /// Whether the mesh's lines are drawn over the map.
    mesh_lines_drawn: bool,
    /// Whether the map is shown in place of the cells: map mode.
    map_mode: bool,
}

impl MapLink {
    /// The map's thread, started, asked nothing yet, not shown: its
    /// cells asked of the host through `terrain`.
    pub fn start_map_link(terrain: impl AsksTerrain + 'static) -> Self {
        let (wanted_sender, maps) = start_map_thread(terrain);
        Self { wanted_sender, maps: Mutex::new(maps), last_asked: None, mesh_lines_drawn: false, map_mode: false }
    }

    /// Whether the map is shown in place of the cells: then none of
    /// them is asked for.
    pub fn map_mode(&self) -> bool {
        self.map_mode
    }
}

/// The map's picture, over the superchunks' images.
#[derive(Component)]
pub struct MapView;

/// The map's picture, hidden until there is one.
pub fn spawn_map_view(mut bevy_commands: Commands, mut images: ResMut<Assets<Image>>) {
    bevy_commands.spawn((Sprite { image: images.add(picture_of((1, 1), DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.5), Visibility::Hidden, MapView));
}

/// Cells along a pixel's side of the map, the view `scale` cells a
/// screen pixel: a power of two, no more than the screen's pixels' --
/// and from near, a cell a pixel.
fn map_step(scale: f32) -> u32 {
    1 << scale.max(1.0).log2().floor() as u32
}

/// Shows the map of the world run in map mode, which `M` turns on and
/// off: asks for one of what the camera shows when the view or the
/// world has changed, and lays the last drawn where it is of. `P`
/// draws the mesh's lines over it, or not.
#[allow(clippy::too_many_arguments)]
pub fn show_map(
    mut map_link: ResMut<MapLink>,
    mut last_seen: ResMut<LastFrameSeen>,
    pressed_keys: Res<ButtonInput<KeyCode>>,
    captured_input: Res<CapturedInput>,
    mut images: ResMut<Assets<Image>>,
    camera_parts: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    map_view: Single<LaidPictureParts, (With<MapView>, Without<Camera2d>)>,
) {
    let (map_sprite, mut map_transform, mut map_visibility) = map_view.into_inner();
    map_link.mesh_lines_drawn ^= pressed_keys.just_pressed(KeyCode::KeyP) && !captured_input.keyboard_keys;
    map_link.map_mode ^= pressed_keys.just_pressed(KeyCode::KeyM) && !captured_input.keyboard_keys;
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    let Some(world) = last_seen.last_frame.as_ref().map(|last_frame| last_frame.world).filter(|_| map_link.map_mode) else {
        map_visibility.set_if_neq(Visibility::Hidden);
        (map_link.last_asked, last_seen.map) = (None, 0);
        return;
    };
    // The view's pixels and a margin, from a corner that is a whole number of pixels: the same cells whatever way it is moved.
    let step = map_step(orthographic_view.scale);
    last_seen.map = step;
    let half_extent = Vec2::new(window.width(), window.height()) * orthographic_view.scale / 2.0;
    let corner_pixel = |cartesian_axis: usize, middle_translation: f32, half_axis_extent: f32| (origin(cartesian_axis) as i64 + (middle_translation - half_axis_extent).floor() as i64).div_euclid(step as i64) - MAP_MARGIN_PIXELS;
    let first_pixel_cartesian = (corner_pixel(0, camera_transform.translation.x, half_extent.x), corner_pixel(1, -camera_transform.translation.y, half_extent.y));
    let pixels_along = |half_along: f32| (2.0 * half_along / step as f32).ceil() as u32 + 2 * MAP_MARGIN_PIXELS as u32 + 1;
    let wanted_map = WantedMap { first_cell_cartesian: (first_pixel_cartesian.0 * step as i64, first_pixel_cartesian.1 * step as i64), step, size_pixels: (pixels_along(half_extent.x), pixels_along(half_extent.y)), world, mesh_lines_drawn: map_link.mesh_lines_drawn };
    // Not for every pixel the view moves: only once it is half the margin from what was asked for, or anything else differs.
    let near_enough = |last_asked_map: &WantedMap| {
        let moved_little = |asked_corner: i64, wanted_corner: i64| (asked_corner - wanted_corner).abs() / step as i64 <= MAP_MARGIN_PIXELS / 2;
        (last_asked_map.step, last_asked_map.size_pixels, last_asked_map.world, last_asked_map.mesh_lines_drawn) == (wanted_map.step, wanted_map.size_pixels, wanted_map.world, wanted_map.mesh_lines_drawn) && moved_little(last_asked_map.first_cell_cartesian.0, wanted_map.first_cell_cartesian.0) && moved_little(last_asked_map.first_cell_cartesian.1, wanted_map.first_cell_cartesian.1)
    };
    if !map_link.last_asked.as_ref().is_some_and(near_enough) && map_link.wanted_sender.send(wanted_map).is_ok() {
        map_link.last_asked = Some(wanted_map);
    }
    let latest_drawn = map_link.maps.lock().expect("the maps' receiver").try_iter().last();
    if let Some(drawn_map) = latest_drawn {
        let (drawn_wanted, drawn_size_pixels) = (drawn_map.wanted_map, drawn_map.wanted_map.size_pixels);
        if let Some(mut image) = images.get_mut(&map_sprite.image) {
            *image = picture_of(drawn_size_pixels, drawn_map.rgba_bytes);
        }
        // Over the cells it is of: a pixel `step` units.
        let plane_middle = |laid_axis: usize, first_cell: i64, pixel_count: u32| (first_cell - origin(laid_axis) as i64) as f32 + (pixel_count * drawn_wanted.step) as f32 / 2.0;
        *map_transform = Transform::from_xyz(plane_middle(0, drawn_wanted.first_cell_cartesian.0, drawn_size_pixels.0), -plane_middle(1, drawn_wanted.first_cell_cartesian.1, drawn_size_pixels.1), 1.5).with_scale(Vec3::new(drawn_wanted.step as f32, drawn_wanted.step as f32, 1.0));
        map_visibility.set_if_neq(Visibility::Visible);
    }
}
