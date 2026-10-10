//! The map: the world as generated, shown in place of its cells in
//! map mode, its cells asked of the host and drawn on a thread of its
//! own (`docs/renderer.md`, "The map").

use crate::frames::{picture_of, Laid, DIRT};
use crate::link::Seen;
use crate::ground::relief::{laid, slope_light, tint, tint_on_sand, water_light, FOAM, FOAM_MOST, PALE, SAND, SAND_MOST};
use crate::paint::{tree_colour, BROWN, GREEN, WATER};
use crate::view::origin;
use bevy::prelude::*;
use gui::Captured;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use server::host::terrain::{Cover, MapAsk, TerrainAsker};

/// Pixels of the map past each of the view's edges: what a moving view
/// shows before the next map comes.
const MARGIN: i64 = 64;
/// How much of its light a pixel on a line of the mesh keeps.
const BORDER_LIGHT: f32 = 0.25;

/// A map asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanted {
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size: (u32, u32),
    /// The world it is of, as the host numbers those it runs.
    pub world: u64,
    /// Whether the mesh's lines are drawn over it.
    pub borders: bool,
}

/// A map drawn.
pub struct Drawn {
    /// What was asked for.
    pub wanted: Wanted,
    /// Its pixels, red, green, blue and opacity, row by row.
    pub pixels: Vec<u8>,
}

/// Starts the map's thread: where to ask for a map, and where it
/// comes back, its cells asked of the host through `terrain`. Of
/// several asked for while one is drawn, only the last is drawn next;
/// one of a world the host runs no more is not drawn.
pub fn start(terrain: TerrainAsker) -> (Sender<Wanted>, Receiver<Drawn>) {
    let (requests, asked) = channel::<Wanted>();
    let (answers, maps) = channel();
    thread::Builder::new()
        .name("map".to_string())
        .spawn(move || {
            while let Ok(first) = asked.recv() {
                let wanted = asked.try_iter().last().unwrap_or(first);
                if let Some(pixels) = draw(&terrain, &wanted)
                    && answers.send(Drawn { wanted, pixels }).is_err()
                {
                    return;
                }
            }
        })
        .expect("a thread for the map");
    (requests, maps)
}

/// The pixels of `wanted`, its cells asked of the host through
/// `terrain`: none if it runs another world by now.
pub fn draw(terrain: &TerrainAsker, wanted: &Wanted) -> Option<Vec<u8>> {
    let answer = terrain.map(MapAsk { world: wanted.world, first: wanted.first, step: wanted.step, size: wanted.size, borders: wanted.borders })?;
    let (width, levels) = (wanted.size.0 as usize, answer.levels);
    let mut pixels = vec![0u8; answer.cells.len() * 4];
    for (cells, pixels) in answer.cells.chunks(width.max(1)).zip(pixels.chunks_mut(width.max(1) * 4)) {
        // The height of the pixel to the left of each.
        let mut before = None;
        for (cell, pixel) in cells.iter().zip(pixels.chunks_mut(4)) {
            let Some(cell) = cell else {
                pixel.copy_from_slice(&[0, 0, 0, u8::MAX]);
                before = None;
                continue;
            };
            // The pixels up and to the left of it, the sun's side: what its slope is told by, and whether the coast passes between.
            let (high, above, beside) = (cell.height, cell.above, before.unwrap_or(cell.height));
            let coast = |ocean: bool| (above < levels.ocean) == ocean || (beside < levels.ocean) == ocean;
            let colour = match cell.cover {
                Cover::Ocean => WATER,
                Cover::Tree => tree_colour(8),
                Cover::Grass => GREEN,
                Cover::Dirt => BROWN,
            };
            let (colour, light) = if cell.cover == Cover::Ocean {
                (if coast(false) { laid(colour, FOAM, FOAM_MOST) } else { colour }, water_light((levels.ocean - high) as f32, levels.ocean.saturating_sub(levels.ground) as f32))
            } else {
                let (tint, pale) = tint(high.saturating_sub(levels.ocean) as f32 / levels.highest.saturating_sub(levels.ocean).max(1) as f32);
                let (colour, tint) = if coast(true) { (laid(colour, SAND, SAND_MOST), tint_on_sand(tint, SAND_MOST)) } else { (laid(colour, PALE, pale), tint) };
                let slope = slope_light((high as f32 - beside as f32) / wanted.step as f32, (high as f32 - above as f32) / wanted.step as f32);
                (colour, tint.map(|tint| tint * slope))
            };
            before = Some(high);
            // A line of the mesh, where the pixel is no farther from it than it is across.
            let light = if cell.on_a_mesh_line { light.map(|light| light * BORDER_LIGHT) } else { light };
            let lit: [u8; 3] = std::array::from_fn(|channel| (colour[channel] as f32 * light[channel]).min(255.0) as u8);
            pixel.copy_from_slice(&[lit[0], lit[1], lit[2], u8::MAX]);
        }
    }
    Some(pixels)
}

/// The map, as the window holds it: where to ask for one, where it
/// comes back, what was last asked for, and whether it is shown.
#[derive(Resource)]
pub struct MapLink {
    /// Where the maps wanted go.
    requests: Sender<Wanted>,
    /// Where they come back, drawn.
    maps: Mutex<Receiver<Drawn>>,
    /// The last asked for.
    asked: Option<Wanted>,
    /// Whether the mesh's lines are drawn over the map.
    borders: bool,
    /// Whether the map is shown in place of the cells: map mode.
    map_mode: bool,
}

impl MapLink {
    /// The map's thread, started, asked nothing yet, not shown: its
    /// cells asked of the host through `terrain`.
    pub fn start(terrain: TerrainAsker) -> Self {
        let (requests, maps) = start(terrain);
        Self { requests, maps: Mutex::new(maps), asked: None, borders: false, map_mode: false }
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
pub fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((Sprite { image: images.add(picture_of((1, 1), DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.5), Visibility::Hidden, MapView));
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
pub fn far(
    mut link: ResMut<MapLink>,
    mut seen: ResMut<Seen>,
    keys: Res<ButtonInput<KeyCode>>,
    captured: Res<Captured>,
    mut images: ResMut<Assets<Image>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    map_view: Single<Laid, (With<MapView>, Without<Camera2d>)>,
) {
    let (sprite, mut transform, mut visibility) = map_view.into_inner();
    link.borders ^= keys.just_pressed(KeyCode::KeyP) && !captured.keys;
    link.map_mode ^= keys.just_pressed(KeyCode::KeyM) && !captured.keys;
    let (camera, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let Some(world) = seen.frame.as_ref().map(|frame| frame.world).filter(|_| link.map_mode) else {
        visibility.set_if_neq(Visibility::Hidden);
        (link.asked, seen.map) = (None, 0);
        return;
    };
    // The view's pixels and a margin, from a corner that is a whole number of pixels: the same cells whatever way it is moved.
    let step = map_step(view.scale);
    seen.map = step;
    let half = Vec2::new(window.width(), window.height()) * view.scale / 2.0;
    let corner = |axis: usize, middle: f32, half: f32| (origin(axis) as i64 + (middle - half).floor() as i64).div_euclid(step as i64) - MARGIN;
    let first = (corner(0, camera.translation.x, half.x), corner(1, -camera.translation.y, half.y));
    let pixels = |half: f32| (2.0 * half / step as f32).ceil() as u32 + 2 * MARGIN as u32 + 1;
    let wanted = Wanted { first: (first.0 * step as i64, first.1 * step as i64), step, size: (pixels(half.x), pixels(half.y)), world, borders: link.borders };
    // Not for every pixel the view moves: only once it is half the margin from what was asked for, or anything else differs.
    let near_enough = |asked: &Wanted| {
        let moved = |asked: i64, wanted: i64| (asked - wanted).abs() / step as i64 <= MARGIN / 2;
        (asked.step, asked.size, asked.world, asked.borders) == (wanted.step, wanted.size, wanted.world, wanted.borders) && moved(asked.first.0, wanted.first.0) && moved(asked.first.1, wanted.first.1)
    };
    if !link.asked.as_ref().is_some_and(near_enough) && link.requests.send(wanted).is_ok() {
        link.asked = Some(wanted);
    }
    let drawn = link.maps.lock().expect("the maps' receiver").try_iter().last();
    if let Some(drawn) = drawn {
        let (wanted, size) = (drawn.wanted, drawn.wanted.size);
        if let Some(mut image) = images.get_mut(&sprite.image) {
            *image = picture_of(size, drawn.pixels);
        }
        // Over the cells it is of: a pixel `step` units.
        let plane = |axis: usize, first: i64, pixels: u32| (first - origin(axis) as i64) as f32 + (pixels * wanted.step) as f32 / 2.0;
        *transform = Transform::from_xyz(plane(0, wanted.first.0, size.0), -plane(1, wanted.first.1, size.1), 1.5).with_scale(Vec3::new(wanted.step as f32, wanted.step as f32, 1.0));
        visibility.set_if_neq(Visibility::Visible);
    }
}
