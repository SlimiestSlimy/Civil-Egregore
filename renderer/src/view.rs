//! The view: the plane the world is laid on ([`Sprites`]), the camera
//! over it, where it starts for each world, and how it is moved and
//! zoomed.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use gui::Captured;
use std::collections::HashMap;

/// A superchunk's side on the screen's plane: a cell a unit.
pub const SPRITE_SIDE: f32 = SUPERCHUNK_SIDE_CELLS as f32;

/// Screen heights the view moves a second, by the keys.
const PAN_SPEED: f32 = 0.8;
/// How much nearer or farther a second, by the keys.
const ZOOM_SPEED: f32 = 2.0;
/// How much nearer a notch of the wheel.
const WHEEL_ZOOM: f32 = 0.85;
/// The farthest a world is first seen from: cells a screen pixel.
/// Some 60 superchunks across a screen.
const FIRST_FARTHEST: f32 = 32.0;
/// The farthest the view goes: cells a screen pixel, four superchunks.
const FARTHEST: f32 = 4096.0;
/// Superchunks along the side of what a world of no size is first
/// seen of: its origin's halo.
const UNLIMITED_SEEN: u32 = 3;

/// The world as drawn: an image a superchunk that has been in the
/// viewport,
/// made when its first pixels come, on a plane counted from the top
/// left of the world's origin superchunk -- the world being too wide
/// for a float's numbers.
#[derive(Resource, Default)]
pub struct Sprites {
    /// Each superchunk drawn, by where it is in the world: its sprite,
    /// its image, and the pixels along the image's side.
    pub tiles: HashMap<(u32, u32), (Entity, Handle<Image>, u32)>,
}

/// The plane's origin, in cells from the world's edge, across (`axis`
/// 0) or down (1).
pub fn origin(axis: usize) -> u32 {
    [WORLD_MIDDLE.cartesian().0, WORLD_MIDDLE.cartesian().1][axis] * SUPERCHUNK_SIDE_CELLS
}

impl Sprites {
    /// Where on the plane the cell `cell` cells from the world's edge
    /// is, across (`axis` 0) or down (1).
    pub fn plane(&self, cell: u32, axis: usize) -> f32 {
        cell.wrapping_sub(origin(axis)) as i32 as f32
    }

    /// The cell at `plane` on the plane, across or down: the world's
    /// first or last if it is past an edge.
    pub fn cell(&self, plane: f32, axis: usize) -> u32 {
        (origin(axis) as i64 + plane.floor() as i64).clamp(0, u32::MAX as i64) as u32
    }

    /// The cells of the viewport -- what the camera shows -- across
    /// and down: the first and the last.
    pub fn viewport_cells(&self, transform: &Transform, scale: f32, window: &Window) -> [(u32, u32); 2] {
        let half = Vec2::new(window.width(), window.height()) * scale / 2.0;
        let middle = [transform.translation.x, -transform.translation.y];
        [0, 1].map(|axis| (self.cell(middle[axis] - half[axis], axis), self.cell(middle[axis] + half[axis], axis)))
    }
}

/// The camera, over the world's origin until a world is seen.
pub fn spawn(mut commands: Commands) {
    commands.spawn((Camera2d, Projection::Orthographic(OrthographicProjection::default_2d())));
}

/// Puts the view over the whole of a world `side` superchunks along a
/// side -- of one of no size, its origin's halo: cells a screen pixel,
/// and its middle on the plane.
pub fn first_view(side: Option<u32>, window: &Window, transform: &mut Transform, projection: &mut Projection) {
    let side = side.unwrap_or(UNLIMITED_SEEN);
    // The world spans `side / 2` superchunks before the origin's and the rest from it: its middle half a superchunk on if it is odd.
    let middle = (side % 2) as f32 * SPRITE_SIDE / 2.0;
    let seen = side as f32 * SPRITE_SIDE;
    if let Projection::Orthographic(view) = projection {
        view.scale = (seen / window.height().min(window.width())).min(FIRST_FARTHEST);
    }
    transform.translation = Vec3::new(middle, -middle, 0.0);
}

/// Moves and zooms the view: the keys, the wheel, and dragging --
/// unless the menus have them.
#[allow(clippy::too_many_arguments)]
pub fn steer(
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    time: Res<Time>,
    captured: Res<Captured>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(view) = &mut *projection else {
        return;
    };
    let held = |these: [KeyCode; 2]| (keys.any_pressed(these) && !captured.keys) as i32 as f32;
    let nearer = held([KeyCode::KeyE, KeyCode::Equal]) - held([KeyCode::KeyQ, KeyCode::Minus]);
    let wheel = if captured.wheel { 0.0 } else { scroll.delta.y };
    view.scale *= WHEEL_ZOOM.powf(wheel) * ZOOM_SPEED.powf(-nearer * time.delta_secs());
    view.scale = view.scale.clamp(0.02, FARTHEST);
    let across = held([KeyCode::KeyD, KeyCode::ArrowRight]) - held([KeyCode::KeyA, KeyCode::ArrowLeft]);
    let up = held([KeyCode::KeyW, KeyCode::ArrowUp]) - held([KeyCode::KeyS, KeyCode::ArrowDown]);
    let step = PAN_SPEED * window.height() * view.scale * time.delta_secs();
    transform.translation += Vec3::new(across * step, up * step, 0.0);
    if buttons.pressed(MouseButton::Left) && !captured.pointer {
        // The world follows the pointer.
        transform.translation += Vec3::new(-motion.delta.x, motion.delta.y, 0.0) * view.scale;
    }
}

/// Puts the window over the whole screen, or back, by its key.
pub fn fullscreen(keys: Res<ButtonInput<KeyCode>>, mut window: Single<&mut Window>, captured: Res<Captured>) {
    if keys.just_pressed(KeyCode::F11) && !captured.keys {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}
