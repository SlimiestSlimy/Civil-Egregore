//! What is drawn over the world, each by its key: the superchunks' and
//! the chunks' boundaries ([`lay_boundaries`]), their names in their
//! corners ([`label_corners`]), and every cell's height on it ([`heights`]).

use crate::renderer::link::{HostLink, LastFrameSeen};
use server::host::terrain::{Height, HeightsAsk};
use crate::renderer::view::{SuperchunkSprites, SPRITE_SIDE};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use coordinates::{place_from_cartesian, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNK_SIDE, SUPERCHUNK_SIDE};
use crate::gui::CapturedInput;

/// Screen pixels two boundaries are apart before they are drawn.
const LINES_FROM: f32 = 6.0;
/// Boundaries of a kind, across or down, there are sprites for: as
/// many as a screen holds [`LINES_FROM`] apart.
const LINES: u32 = 700;
/// A superchunks' boundary's colour, and its width in screen pixels.
const SUPERCHUNK_LINE: (Color, f32) = (Color::srgba(1.0, 0.85, 0.2, 0.9), 2.0);
/// A chunks' boundary's.
const CHUNK_LINE: (Color, f32) = (Color::srgba(1.0, 1.0, 1.0, 0.45), 1.0);
/// Labels there are: what a view can hold of them.
const CORNER_LABEL_COUNT: usize = 256;
/// Screen pixels a superchunk or a chunk is across before it is labelled.
const LABELLED_FROM: f32 = 150.0;
/// Screen pixels a label is across at its full size: under that much
/// room it is drawn smaller, to fit.
const LABEL_WIDTH: f32 = 420.0;
/// Screen pixels from a corner to its label, and from one line of
/// labels to the next.
const LABEL_LINE: f32 = 24.0;
/// Height labels across and down: the most cells the camera shows that are labelled.
const HEIGHT_LABELS: (u32, u32) = (96, 54);
/// Screen pixels a cell is across before its height is written on it.
const HEIGHT_FROM: f32 = 20.0;
/// Screen pixels a height is across at its full size: five digits.
const HEIGHT_WIDTH: f32 = 80.0;

/// A boundary drawn over the world: a line between two rows or two
/// columns of chunks, from one side of the world to the other.
#[derive(Component)]
pub struct DrawnBoundary {
    /// Whether it is one of the superchunks' lines, or of the chunks'.
    of_superchunks: bool,
    /// Whether it runs across, or down.
    runs_across: bool,
    /// Which of those the camera shows it is, counted from the first.
    place: u32,
}

/// Which are shown.
#[derive(Resource, Default)]
pub struct OverlaysShown {
    /// The superchunks' boundaries.
    superchunks: bool,
    /// The chunks'.
    chunks: bool,
    /// Every cell's height.
    heights: bool,
    /// Every tile's climate (`climate_labels`).
    pub(crate) climates: bool,
}

/// A label in a superchunk's or a chunk's top left corner: one of a
/// few, given to those the camera shows.
#[derive(Component)]
pub struct CornerLabel;

/// A cell's height, written on it: one of a grid of them, each given
/// to the cell the camera shows whose `(x, y)` is its own, counted round the
/// grid -- so a cell keeps its text while the view moves.
#[derive(Component)]
pub struct HeightLabel {
    /// Where it is in the grid.
    slot: (u32, u32),
}

/// The camera, and none of these: both have a place.
type CameraOnly = (With<Camera2d>, Without<CornerLabel>, Without<HeightLabel>, Without<DrawnBoundary>);

/// The boundaries and the labels, hidden until asked for.
pub fn spawn_overlays(mut bevy_commands: Commands) {
    // The chunks' boundaries under the superchunks'.
    for (of_superchunks, (boundary_colour, _), height) in [(false, CHUNK_LINE, 2.0), (true, SUPERCHUNK_LINE, 3.0)] {
        for place in 0..LINES {
            for runs_across in [true, false] {
                let boundary_sprite = Sprite { color: boundary_colour, custom_size: Some(Vec2::ONE), ..default() };
                bevy_commands.spawn((boundary_sprite, Transform::from_xyz(0.0, 0.0, height), Visibility::Hidden, DrawnBoundary { of_superchunks, runs_across, place }));
            }
        }
    }
    for _ in 0..CORNER_LABEL_COUNT {
        bevy_commands.spawn((Text2d::new(""), Anchor::TOP_LEFT, Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, CornerLabel));
    }
    for slot in 0..HEIGHT_LABELS.0 * HEIGHT_LABELS.1 {
        bevy_commands.spawn((Text2d::new(""), Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, HeightLabel { slot: (slot % HEIGHT_LABELS.0, slot / HEIGHT_LABELS.0) }));
    }
}

/// Shows and hides each by its key.
pub fn toggle(mut overlays_shown: ResMut<OverlaysShown>, pressed_keys: Res<ButtonInput<KeyCode>>, captured_input: Res<CapturedInput>) {
    if !captured_input.keyboard_keys {
        overlays_shown.superchunks ^= pressed_keys.just_pressed(KeyCode::KeyB);
        overlays_shown.chunks ^= pressed_keys.just_pressed(KeyCode::KeyC);
        overlays_shown.heights ^= pressed_keys.just_pressed(KeyCode::KeyH);
        overlays_shown.climates ^= pressed_keys.just_pressed(KeyCode::KeyK);
    }
}

/// Lays the boundaries shown over the lines the camera shows, as wide on the
/// screen however near the view is -- once the lines are far enough
/// apart on it to be told apart.
pub fn lay_boundaries(overlays_shown: Res<OverlaysShown>, mut lines: Query<(&DrawnBoundary, &mut Transform, &mut Visibility)>, camera_parts: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>) {
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    let half_extent = Vec2::new(window.width(), window.height()) * orthographic_view.scale / 2.0;
    // The view's middle on the plane, across and down.
    let middle_translation = Vec2::new(camera_transform.translation.x, -camera_transform.translation.y);
    for (drawn_boundary, mut boundary_transform, mut boundary_visibility) in &mut lines {
        let (is_shown, (_, width_pixels), apart_cells) = if drawn_boundary.of_superchunks { (overlays_shown.superchunks, SUPERCHUNK_LINE, SPRITE_SIDE) } else { (overlays_shown.chunks, CHUNK_LINE, CHUNK_SIDE as f32) };
        // A line across is one of those down the view, and the other way round.
        let (along_axis, across_axis) = if drawn_boundary.runs_across { (1, 0) } else { (0, 1) };
        let boundary_translation = (((middle_translation[along_axis] - half_extent[along_axis]) / apart_cells).ceil() + drawn_boundary.place as f32) * apart_cells;
        if !is_shown || apart_cells / orthographic_view.scale < LINES_FROM || boundary_translation > middle_translation[along_axis] + half_extent[along_axis] {
            *boundary_visibility = Visibility::Hidden;
            continue;
        }
        let (width, boundary_length) = (width_pixels * orthographic_view.scale, 2.0 * half_extent[across_axis]);
        *boundary_transform = if drawn_boundary.runs_across {
            Transform::from_xyz(middle_translation.x, -boundary_translation, boundary_transform.translation.z).with_scale(Vec3::new(boundary_length, width, 1.0))
        } else {
            Transform::from_xyz(boundary_translation, -middle_translation.y, boundary_transform.translation.z).with_scale(Vec3::new(width, boundary_length, 1.0))
        };
        *boundary_visibility = Visibility::Visible;
    }
}

/// The heights last asked of the host for the height labels: the
/// world and the first cell they are of, and the heights.
type HeightsAsked = Option<((u64, (u32, u32)), Vec<Height>)>;

/// Writes every cell's height on it, as the world run is generated,
/// once the cells are large enough on the screen to read it and few
/// enough for the labels there are.
#[allow(clippy::too_many_arguments)]
pub fn heights(overlays_shown: Res<OverlaysShown>, superchunk_sprites: Res<SuperchunkSprites>, last_seen: Res<LastFrameSeen>, host_link: Res<HostLink>, mut heights_asked: Local<HeightsAsked>, camera_parts: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut height_labels: Query<(&HeightLabel, &mut Text2d, &mut Transform, &mut Visibility)>) {
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    let [(first_x, last_x), (first_y, last_y)] = superchunk_sprites.viewport_cells(camera_transform, orthographic_view.scale, &window);
    let (first_cell_cartesian, last_cell_cartesian) = ((first_x, first_y), (last_x, last_y));
    let heights_readable = overlays_shown.heights && 1.0 / orthographic_view.scale >= HEIGHT_FROM && last_cell_cartesian.0 - first_cell_cartesian.0 < HEIGHT_LABELS.0 && last_cell_cartesian.1 - first_cell_cartesian.1 < HEIGHT_LABELS.1;
    let label_scale = orthographic_view.scale * (1.0 / orthographic_view.scale / HEIGHT_WIDTH).min(1.0);
    let world = last_seen.last_frame.as_ref().filter(|_| heights_readable).map(|last_frame| last_frame.world);
    // The heights of the cells the labels may stand on, asked of the host once for a view: again only when it has moved, or the world is another.
    if let Some(world) = world
        && heights_asked.as_ref().is_none_or(|(asked_of, _)| *asked_of != (world, first_cell_cartesian))
    {
        *heights_asked = host_link.host.terrain().heights(HeightsAsk { world, first_cell_cartesian, size_in_cells: HEIGHT_LABELS, skipped_corners_cartesian: Vec::new() }).map(|heights| ((world, first_cell_cartesian), heights));
    }
    let shown_heights = world.and(heights_asked.as_ref()).filter(|(kept_of, _)| kept_of.1 == first_cell_cartesian).map(|(_, heights)| heights);
    for (height_label, mut label_text, mut label_transform, mut label_visibility) in &mut height_labels {
        // The cell the camera shows that is the label's: the first at or past the view's first whose place round the grid is its slot.
        let round_to_slot = |first_along: u32, slot: u32, labels_along: u32| first_along + (slot + labels_along - first_along % labels_along) % labels_along;
        let (x, y) = (round_to_slot(first_cell_cartesian.0, height_label.slot.0, HEIGHT_LABELS.0), round_to_slot(first_cell_cartesian.1, height_label.slot.1, HEIGHT_LABELS.1));
        let Some(heights) = shown_heights.filter(|_| x <= last_cell_cartesian.0 && y <= last_cell_cartesian.1) else {
            *label_visibility = Visibility::Hidden;
            continue;
        };
        let height = heights[((y - first_cell_cartesian.1) * HEIGHT_LABELS.0 + x - first_cell_cartesian.0) as usize].to_string();
        if label_text.0 != height {
            label_text.0 = height;
        }
        *label_transform = Transform::from_xyz(superchunk_sprites.translation_of_cell(x, 0) + 0.5, -(superchunk_sprites.translation_of_cell(y, 1) + 0.5), 4.0).with_scale(Vec3::splat(label_scale));
        *label_visibility = Visibility::Visible;
    }
}

/// Labels the superchunks and chunks the camera shows whose boundaries are
/// shown, once they are large enough on the screen: each one's Morton
/// index and its `(x, y)`, in its top left corner -- a chunk's a line
/// below, clear of its superchunk's.
pub fn label_corners(overlays_shown: Res<OverlaysShown>, superchunk_sprites: Res<SuperchunkSprites>, camera_parts: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut corner_labels: Query<(&mut Text2d, &mut Transform, &mut Visibility), With<CornerLabel>>) {
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    // The chunks the camera shows, counted from the world's top left: every label is at a chunk's corner.
    let [(first_x, last_x), (first_y, last_y)] = superchunk_sprites.viewport_cells(camera_transform, orthographic_view.scale, &window).map(|(first_cell_along, last_cell_along)| (first_cell_along / CHUNK_SIDE as u32, last_cell_along / CHUNK_SIDE as u32));
    // How large a label of something `cells` across is drawn, of its full size: none if there is no room to read it.
    let fitted_scale = |cells: usize| Some(cells as f32 / orthographic_view.scale).filter(|&room_pixels| room_pixels >= LABELLED_FROM).map(|fitted_room| (fitted_room / LABEL_WIDTH).min(1.0));
    let (superchunk_size, chunk_size) = (fitted_scale(SPRITE_SIDE as usize).filter(|_| overlays_shown.superchunks), fitted_scale(CHUNK_SIDE).filter(|_| overlays_shown.chunks));
    let mut wanted_labels = Vec::new();
    for y in first_y..=last_y {
        for x in first_x..=last_x {
            let (within_x, within_y) = (x % SUPERCHUNK_SIDE as u32, y % SUPERCHUNK_SIDE as u32);
            let superchunk = SuperchunkIndex::from_cartesian(x / SUPERCHUNK_SIDE as u32, y / SUPERCHUNK_SIDE as u32);
            let (superchunk_x, superchunk_y) = superchunk.cartesian();
            let chunk_corner = Vec2::new(superchunk_sprites.translation_of_cell(x * CHUNK_SIDE as u32, 0), -superchunk_sprites.translation_of_cell(y * CHUNK_SIDE as u32, 1));
            if let Some(superchunk_label_scale) = superchunk_size.filter(|_| (within_x, within_y) == (0, 0)) {
                wanted_labels.push((chunk_corner, 0.0, superchunk_label_scale, format!("superchunk {:011x} ({superchunk_x}, {superchunk_y})", superchunk.0)));
            }
            if let Some(chunk_label_scale) = chunk_size {
                let place = place_from_cartesian(within_x * CHUNK_SIDE as u32, within_y * CHUNK_SIDE as u32) / CELLS_IN_CHUNK;
                wanted_labels.push((chunk_corner, superchunk_size.unwrap_or(0.0), chunk_label_scale, format!("chunk {:012x} ({x}, {y})", ChunkIndex::from_superchunk_place(superchunk, place).0)));
            }
        }
    }
    let mut remaining_labels = wanted_labels.into_iter();
    for (mut label_text, mut label_transform, mut label_visibility) in &mut corner_labels {
        let Some((label_corner, lines_above, label_scale, label_words)) = remaining_labels.next() else {
            *label_visibility = Visibility::Hidden;
            continue;
        };
        if label_text.0 != label_words {
            label_text.0 = label_words;
        }
        // As large on the screen however near the view is, and a little in from the corner.
        let corner_inset = Vec2::new(LABEL_LINE / 4.0, -LABEL_LINE / 4.0 - lines_above * LABEL_LINE) * orthographic_view.scale;
        *label_transform = Transform::from_translation((label_corner + corner_inset).extend(4.0)).with_scale(Vec3::splat(orthographic_view.scale * label_scale));
        *label_visibility = Visibility::Visible;
    }
}
