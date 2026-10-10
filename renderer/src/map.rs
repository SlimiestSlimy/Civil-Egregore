//! The map: the world as generated, shown in place of its cells in map
//! mode, which `M` turns on and off, at any zoom. It asks nothing of the
//! simulation: a pixel is the cell in its middle, as that cell is
//! generated -- its height from the seed, the ocean over it or grass,
//! dirt or a tree on it -- so nothing is made hot to be looked at, and
//! the world is seen as far out as islands are specks. A thread of its
//! own draws the last map asked for, on every thread the machine has
//! ([`start`]); the window asks for one of what the camera shows, and
//! lays it where it is of ([`far`]).

use crate::frames::{picture_of, Laid, DIRT};
use crate::link::Seen;
use crate::paint::{tree_colour, BROWN, GREEN, WATER};
use crate::view::origin;
use bevy::prelude::*;
use gui::Captured;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use server::host::terrain_seen::{levels, Cover, CoverSeen, Generation, HeightsSeen};

/// Pixels of the map past each of the view's edges: what a moving view
/// shows before the next map comes.
const MARGIN: i64 = 64;
/// How much of its light the deepest ocean keeps.
const DEEP_LIGHT: f32 = 0.35;
/// How much of its light a pixel on a line of the mesh keeps.
const BORDER_LIGHT: f32 = 0.25;
/// How much lighter or darker a slope of one height a cell is drawn.
const SLOPE_LIGHT: f32 = 2.5;

/// A map asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wanted {
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size: (u32, u32),
    /// The seed of the world it is of.
    pub seed: u64,
    /// How that world is generated.
    pub generation: Generation,
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
/// comes back. Of several asked for while one is drawn, only the last
/// is drawn next.
pub fn start() -> (Sender<Wanted>, Receiver<Drawn>) {
    let (requests, asked) = channel::<Wanted>();
    let (answers, maps) = channel();
    thread::Builder::new()
        .name("map".to_string())
        .spawn(move || {
            while let Ok(first) = asked.recv() {
                let wanted = asked.try_iter().last().unwrap_or(first);
                if answers.send(Drawn { wanted, pixels: draw(&wanted) }).is_err() {
                    return;
                }
            }
        })
        .expect("a thread for the map");
    (requests, maps)
}

/// The cell in the middle of the pixel `(x, y)` of `wanted`, or `None`
/// past the world's edges.
fn cell(wanted: &Wanted, x: i64, y: i64) -> Option<(u32, u32)> {
    let cell = |first: i64, pixel: i64| u32::try_from(first + pixel * wanted.step as i64 + wanted.step as i64 / 2).ok();
    Some((cell(wanted.first.0, x)?, cell(wanted.first.1, y)?))
}

/// The pixels of `wanted`: rows shared out among the machine's threads.
fn draw(wanted: &Wanted) -> Vec<u8> {
    let (width, generation, seed) = (wanted.size.0 as usize, &wanted.generation, wanted.seed);
    let (cover, levels) = (&CoverSeen::of(generation, seed), levels(generation));
    let mut pixels = vec![0u8; width * wanted.size.1 as usize * 4];
    let threads = thread::available_parallelism().map_or(1, |threads| threads.get());
    let rows_each = (wanted.size.1 as usize).div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (part, rows) in pixels.chunks_mut(rows_each * width * 4).enumerate() {
            scope.spawn(move || {
                let mut seen = HeightsSeen::of(generation, seed);
                for (row, pixels) in rows.chunks_mut(width * 4).enumerate() {
                    let y = (part * rows_each + row) as i64;
                    // The height up and to the left of each pixel, the sun's side: what its slope is told by.
                    let mut before = None;
                    for (x, pixel) in pixels.chunks_mut(4).enumerate() {
                        let Some((cell_x, cell_y)) = cell(wanted, x as i64, y) else {
                            pixel.copy_from_slice(&[0, 0, 0, u8::MAX]);
                            before = None;
                            continue;
                        };
                        let high = seen.height(cell_x, cell_y);
                        let (colour, light) = if high < levels.ocean {
                            (WATER, 1.0 - (1.0 - DEEP_LIGHT) * ((levels.ocean - high) as f32 / (levels.ocean - levels.ground).max(1) as f32).min(1.0))
                        } else {
                            let colour = match cover.cover(cell_x, cell_y) {
                                Cover::Tree => tree_colour(8),
                                Cover::Grass => GREEN,
                                Cover::Dirt => BROWN,
                            };
                            let above = cell(wanted, x as i64, y - 1).map_or(high, |(x, y)| seen.height(x, y));
                            let lower = (above as f32 + before.unwrap_or(high) as f32) / 2.0;
                            let slope = (high as f32 - lower) / wanted.step as f32;
                            let tint = 0.8 + 0.3 * (high.saturating_sub(levels.ocean) as f32 / levels.highest.saturating_sub(levels.ocean).max(1) as f32).min(1.0);
                            (colour, tint * (1.0 + SLOPE_LIGHT * slope).clamp(0.55, 1.45))
                        };
                        before = Some(high);
                        // A line of the mesh, where the pixel is no farther from it than it is across.
                        let on_border = wanted.borders && seen.cells_from_a_mesh_line(cell_x, cell_y) < wanted.step as u64;
                        let light = if on_border { light * BORDER_LIGHT } else { light };
                        let lit = colour.map(|channel| (channel as f32 * light).min(255.0) as u8);
                        pixel.copy_from_slice(&[lit[0], lit[1], lit[2], u8::MAX]);
                    }
                }
            });
        }
    });
    pixels
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
    /// The map's thread, started, asked nothing yet, not shown.
    pub fn start() -> Self {
        let (requests, maps) = start();
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
    let Some(world) = seen.frame.as_ref().map(|frame| (frame.seed, frame.generation)).filter(|_| link.map_mode) else {
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
    let wanted = Wanted { first: (first.0 * step as i64, first.1 * step as i64), step, size: (pixels(half.x), pixels(half.y)), seed: world.0, generation: world.1, borders: link.borders };
    // Not for every pixel the view moves: only once it is half the margin from what was asked for, or anything else differs.
    let near_enough = |asked: &Wanted| {
        let moved = |asked: i64, wanted: i64| (asked - wanted).abs() / step as i64 <= MARGIN / 2;
        (asked.step, asked.size, asked.seed, asked.generation, asked.borders) == (wanted.step, wanted.size, wanted.seed, wanted.generation, wanted.borders) && moved(asked.first.0, wanted.first.0) && moved(asked.first.1, wanted.first.1)
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
