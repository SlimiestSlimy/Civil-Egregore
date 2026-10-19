//! The window's camera: where the world is seen from and how far out,
//! moved by the keys and the mouse; and what of the world it sees -- in
//! superchunks, and at which detail level ([`seen_by`]). The world is
//! drawn a cell a unit, about the world's middle: its far corners lie
//! beyond what Bevy's numbers hold finely.

use crate::client_api::rendering::SUPERCHUNK_SIDE_LOG2;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use protocol::pipe::frame::Viewport;
use utilities::coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE_CELL, WORLD_SIDE_SUPERCHUNKS};

/// Cells a pixel as the window opens: a few superchunks across.
const FIRST_CELLS_A_PIXEL: f32 = 4.0;
/// Cells a pixel at the nearest: a cell many pixels.
const NEAREST: f32 = 1.0 / 16.0;
/// Cells a pixel at the farthest: a superchunk a few pixels.
const FARTHEST: f32 = 512.0;
/// The window's widths a second the keys move the camera.
const KEY_PAN_SPEED: f32 = 0.75;
/// How much nearer or farther a line of the mouse wheel takes the camera.
const ZOOM_A_LINE: f32 = 1.25;
/// Lines of the mouse wheel a second Q and E zoom by.
const KEY_ZOOM_LINES: f32 = 6.0;
/// Pixels of a touchpad's scrolling taken as a line of the wheel.
const PIXELS_A_LINE: f32 = 40.0;

/// Where the cell `(x, y)` of the world is drawn: its top left corner.
pub fn drawn_at(x: u64, y: u64) -> Vec2 {
    Vec2::new((x as i64 - i64::from(WORLD_MIDDLE_CELL.x)) as f32, (i64::from(WORLD_MIDDLE_CELL.y) - y as i64) as f32)
}

/// The window's camera, over the world's middle.
pub fn spawn_camera(mut bevy_commands: Commands) {
    bevy_commands.spawn((Camera2d, Projection::Orthographic(OrthographicProjection { scale: FIRST_CELLS_A_PIXEL, ..OrthographicProjection::default_2d() })));
}

/// The camera moved by the keys -- WASD or the arrows -- or dragged by
/// the mouse, and taken nearer or farther by the wheel, about the
/// pointer, or by E and Q, about the window's middle.
pub fn move_camera(keys: Res<ButtonInput<KeyCode>>, mouse_buttons: Res<ButtonInput<MouseButton>>, mouse_motion: Res<AccumulatedMouseMotion>, mouse_scroll: Res<AccumulatedMouseScroll>, time: Res<Time>, window: Single<&Window>, camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>) {
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(orthographic) = &mut *projection else {
        return;
    };
    let pressed = |key_codes: [KeyCode; 2]| key_codes.iter().any(|&key_code| keys.pressed(key_code));
    let key_direction = Vec2::new(f32::from(pressed([KeyCode::KeyD, KeyCode::ArrowRight])) - f32::from(pressed([KeyCode::KeyA, KeyCode::ArrowLeft])), f32::from(pressed([KeyCode::KeyW, KeyCode::ArrowUp])) - f32::from(pressed([KeyCode::KeyS, KeyCode::ArrowDown])));
    let mut moved = key_direction * KEY_PAN_SPEED * window.width() * orthographic.scale * time.delta_secs();
    if mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) {
        moved += Vec2::new(-mouse_motion.delta.x, mouse_motion.delta.y) * orthographic.scale;
    }
    let key_lines = (f32::from(keys.pressed(KeyCode::KeyE)) - f32::from(keys.pressed(KeyCode::KeyQ))) * KEY_ZOOM_LINES * time.delta_secs();
    let lines_scrolled = key_lines
        + match mouse_scroll.unit {
            MouseScrollUnit::Line => mouse_scroll.delta.y,
            MouseScrollUnit::Pixel => mouse_scroll.delta.y / PIXELS_A_LINE,
        };
    if lines_scrolled != 0.0 {
        let new_scale = (orthographic.scale * ZOOM_A_LINE.powf(-lines_scrolled)).clamp(NEAREST, FARTHEST);
        // The cell under the pointer stays under it.
        if let Some(pointer) = window.cursor_position().filter(|_| key_lines == 0.0) {
            let from_middle = Vec2::new(pointer.x - window.width() / 2.0, window.height() / 2.0 - pointer.y);
            moved += from_middle * (orthographic.scale - new_scale);
        }
        orthographic.scale = new_scale;
    }
    transform.translation += moved.extend(0.0);
}

/// What the camera sees of the world, in a window of `window_size`
/// pixels: the superchunks, and the detail level -- a drawn cell about
/// a pixel.
pub fn seen_by(transform: &Transform, orthographic: &OrthographicProjection, window_size: Vec2) -> (Viewport, u32) {
    let middle_cell = Vec2::new(WORLD_MIDDLE_CELL.x as f32 + transform.translation.x, WORLD_MIDDLE_CELL.y as f32 - transform.translation.y);
    let half_seen = window_size / 2.0 * orthographic.scale;
    let superchunk_of = |cell: f32| ((cell / SUPERCHUNK_SIDE_CELLS as f32).floor().max(0.0) as u32).min(WORLD_SIDE_SUPERCHUNKS - 1);
    let top_left = (superchunk_of(middle_cell.x - half_seen.x), superchunk_of(middle_cell.y - half_seen.y));
    let bottom_right = (superchunk_of(middle_cell.x + half_seen.x), superchunk_of(middle_cell.y + half_seen.y));
    let detail_level = (orthographic.scale.max(1.0).log2().floor() as u32).min(SUPERCHUNK_SIDE_LOG2);
    (Viewport { top_left_superchunk_cartesian: top_left, bottom_right_superchunk_cartesian: bottom_right }, detail_level)
}
