//! The view: the plane the world is laid on ([`SuperchunkSprites`]), the camera
//! over it, where it starts for each world, and how it is moved and
//! zoomed.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use gui::CapturedInput;
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
const FARTHEST_CELLS_A_PIXEL: f32 = 4096.0;
/// Superchunks along the side of what a world of no size is first
/// seen of: its origin's halo.
const UNLIMITED_SEEN: u32 = 3;

/// The world as drawn: an image a superchunk that has been in the
/// viewport,
/// made when its first pixels come, on a plane counted from the top
/// left of the world's origin superchunk -- the world being too wide
/// for a float's numbers.
#[derive(Resource, Default)]
pub struct SuperchunkSprites {
    /// Each superchunk drawn, by where it is in the world: its sprite,
    /// its image, and the pixels along the image's side.
    pub tiles: HashMap<(u32, u32), (Entity, Handle<Image>, u32)>,
}

/// The plane's origin, in cells from the world's edge, across (`cartesian_axis`
/// 0) or down (1).
pub fn origin(cartesian_axis: usize) -> u32 {
    [WORLD_MIDDLE.cartesian().0, WORLD_MIDDLE.cartesian().1][cartesian_axis] * SUPERCHUNK_SIDE_CELLS
}

impl SuperchunkSprites {
    /// Where on the plane the cell `cell` cells from the world's edge
    /// is, across (`cartesian_axis` 0) or down (1).
    pub fn translation_of_cell(&self, cell: u32, cartesian_axis: usize) -> f32 {
        cell.wrapping_sub(origin(cartesian_axis)) as i32 as f32
    }

    /// The cell at `translation_on_plane` on the plane, across or down: the world's
    /// first or last if it is past an edge.
    pub fn cell_at_translation(&self, translation_on_plane: f32, cartesian_axis: usize) -> u32 {
        (origin(cartesian_axis) as i64 + translation_on_plane.floor() as i64).clamp(0, u32::MAX as i64) as u32
    }

    /// The cells of the viewport -- what the camera shows -- across
    /// and down: the first and the last.
    pub fn viewport_cells(&self, camera_transform: &Transform, scale: f32, window: &Window) -> [(u32, u32); 2] {
        let half_extent = Vec2::new(window.width(), window.height()) * scale / 2.0;
        let middle_translation = [camera_transform.translation.x, -camera_transform.translation.y];
        [0, 1].map(|cartesian_axis| (self.cell_at_translation(middle_translation[cartesian_axis] - half_extent[cartesian_axis], cartesian_axis), self.cell_at_translation(middle_translation[cartesian_axis] + half_extent[cartesian_axis], cartesian_axis)))
    }
}

/// The camera, over the world's origin until a world is seen.
pub fn spawn_camera(mut bevy_commands: Commands) {
    bevy_commands.spawn((Camera2d, Projection::Orthographic(OrthographicProjection::default_2d())));
}

/// Puts the view over the whole of a world `world_side_superchunks` superchunks along a
/// side -- of one of no size, its origin's halo: cells a screen pixel,
/// and its middle on the plane.
pub fn first_view(world_side_superchunks: Option<u32>, window: &Window, camera_transform: &mut Transform, camera_projection: &mut Projection) {
    let seen_side_superchunks = world_side_superchunks.unwrap_or(UNLIMITED_SEEN);
    // The world spans `world_side_superchunks / 2` superchunks before the origin's and the rest from it: its middle half a superchunk on if it is odd.
    let middle_offset = (seen_side_superchunks % 2) as f32 * SPRITE_SIDE / 2.0;
    let seen_cells = seen_side_superchunks as f32 * SPRITE_SIDE;
    if let Projection::Orthographic(orthographic_view) = camera_projection {
        orthographic_view.scale = (seen_cells / window.height().min(window.width())).min(FIRST_FARTHEST);
    }
    camera_transform.translation = Vec3::new(middle_offset, -middle_offset, 0.0);
}

/// Moves and zooms the view: the keys, the wheel, and dragging --
/// unless the menus have them.
#[allow(clippy::too_many_arguments)]
pub fn steer_view(
    camera_parts: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    pressed_keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    frame_time: Res<Time>,
    captured_input: Res<CapturedInput>,
) {
    let (mut camera_transform, mut camera_projection) = camera_parts.into_inner();
    let Projection::Orthographic(orthographic_view) = &mut *camera_projection else {
        return;
    };
    let keys_held = |key_pair: [KeyCode; 2]| (pressed_keys.any_pressed(key_pair) && !captured_input.keyboard_keys) as i32 as f32;
    let zoom_nearer = keys_held([KeyCode::KeyE, KeyCode::Equal]) - keys_held([KeyCode::KeyQ, KeyCode::Minus]);
    let wheel = if captured_input.wheel { 0.0 } else { mouse_scroll.delta.y };
    orthographic_view.scale *= WHEEL_ZOOM.powf(wheel) * ZOOM_SPEED.powf(-zoom_nearer * frame_time.delta_secs());
    orthographic_view.scale = orthographic_view.scale.clamp(0.02, FARTHEST_CELLS_A_PIXEL);
    let move_across = keys_held([KeyCode::KeyD, KeyCode::ArrowRight]) - keys_held([KeyCode::KeyA, KeyCode::ArrowLeft]);
    let move_up = keys_held([KeyCode::KeyW, KeyCode::ArrowUp]) - keys_held([KeyCode::KeyS, KeyCode::ArrowDown]);
    let step = PAN_SPEED * window.height() * orthographic_view.scale * frame_time.delta_secs();
    camera_transform.translation += Vec3::new(move_across * step, move_up * step, 0.0);
    if mouse_buttons.pressed(MouseButton::Left) && !captured_input.pointer_buttons {
        // The world follows the pointer.
        camera_transform.translation += Vec3::new(-mouse_motion.delta.x, mouse_motion.delta.y, 0.0) * orthographic_view.scale;
    }
}

/// Puts the window over the whole screen, or back, by its key.
pub fn toggle_fullscreen(pressed_keys: Res<ButtonInput<KeyCode>>, mut window: Single<&mut Window>, captured_input: Res<CapturedInput>) {
    if pressed_keys.just_pressed(KeyCode::F11) && !captured_input.keyboard_keys {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}
