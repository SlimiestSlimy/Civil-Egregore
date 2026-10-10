//! What is drawn over the world, each by its key: the superchunks' and
//! the chunks' boundaries ([`boundaries`]), their names in their
//! corners ([`labels`]), and every cell's height on it ([`heights`]).

use crate::link::{Link, Seen};
use server::host::terrain::{Height, HeightsAsk};
use crate::view::{Sprites, SPRITE_SIDE};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use coordinates::{place_from_cartesian, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNK_SIDE, SUPERCHUNK_SIDE};
use gui::Captured;

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
const LABELS: usize = 256;
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
pub struct Boundary {
    /// Whether it is one of the superchunks' lines, or of the chunks'.
    of_superchunks: bool,
    /// Whether it runs across, or down.
    across: bool,
    /// Which of those the camera shows it is, counted from the first.
    place: u32,
}

/// Which are shown.
#[derive(Resource, Default)]
pub struct Shown {
    /// The superchunks' boundaries.
    superchunks: bool,
    /// The chunks'.
    chunks: bool,
    /// Every cell's height.
    heights: bool,
}

/// A label in a superchunk's or a chunk's top left corner: one of a
/// few, given to those the camera shows.
#[derive(Component)]
pub struct Label;

/// A cell's height, written on it: one of a grid of them, each given
/// to the cell the camera shows whose `(x, y)` is its own, counted round the
/// grid -- so a cell keeps its text while the view moves.
#[derive(Component)]
pub struct HeightLabel {
    /// Where it is in the grid.
    slot: (u32, u32),
}

/// The camera, and none of these: both have a place.
type CameraOnly = (With<Camera2d>, Without<Label>, Without<HeightLabel>, Without<Boundary>);

/// The boundaries and the labels, hidden until asked for.
pub fn spawn(mut commands: Commands) {
    // The chunks' boundaries under the superchunks'.
    for (of_superchunks, (colour, _), height) in [(false, CHUNK_LINE, 2.0), (true, SUPERCHUNK_LINE, 3.0)] {
        for place in 0..LINES {
            for across in [true, false] {
                let sprite = Sprite { color: colour, custom_size: Some(Vec2::ONE), ..default() };
                commands.spawn((sprite, Transform::from_xyz(0.0, 0.0, height), Visibility::Hidden, Boundary { of_superchunks, across, place }));
            }
        }
    }
    for _ in 0..LABELS {
        commands.spawn((Text2d::new(""), Anchor::TOP_LEFT, Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, Label));
    }
    for slot in 0..HEIGHT_LABELS.0 * HEIGHT_LABELS.1 {
        commands.spawn((Text2d::new(""), Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, HeightLabel { slot: (slot % HEIGHT_LABELS.0, slot / HEIGHT_LABELS.0) }));
    }
}

/// Shows and hides each by its key.
pub fn toggle(mut shown: ResMut<Shown>, keys: Res<ButtonInput<KeyCode>>, captured: Res<Captured>) {
    if !captured.keys {
        shown.superchunks ^= keys.just_pressed(KeyCode::KeyB);
        shown.chunks ^= keys.just_pressed(KeyCode::KeyC);
        shown.heights ^= keys.just_pressed(KeyCode::KeyH);
    }
}

/// Lays the boundaries shown over the lines the camera shows, as wide on the
/// screen however near the view is -- once the lines are far enough
/// apart on it to be told apart.
pub fn boundaries(shown: Res<Shown>, mut lines: Query<(&Boundary, &mut Transform, &mut Visibility)>, camera: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>) {
    let (camera, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let half = Vec2::new(window.width(), window.height()) * view.scale / 2.0;
    // The view's middle on the plane, across and down.
    let middle = Vec2::new(camera.translation.x, -camera.translation.y);
    for (boundary, mut transform, mut visibility) in &mut lines {
        let (is_shown, (_, pixels), apart) = if boundary.of_superchunks { (shown.superchunks, SUPERCHUNK_LINE, SPRITE_SIDE) } else { (shown.chunks, CHUNK_LINE, CHUNK_SIDE as f32) };
        // A line across is one of those down the view, and the other way round.
        let (along, over) = if boundary.across { (1, 0) } else { (0, 1) };
        let at = (((middle[along] - half[along]) / apart).ceil() + boundary.place as f32) * apart;
        if !is_shown || apart / view.scale < LINES_FROM || at > middle[along] + half[along] {
            *visibility = Visibility::Hidden;
            continue;
        }
        let (width, length) = (pixels * view.scale, 2.0 * half[over]);
        *transform = if boundary.across {
            Transform::from_xyz(middle.x, -at, transform.translation.z).with_scale(Vec3::new(length, width, 1.0))
        } else {
            Transform::from_xyz(at, -middle.y, transform.translation.z).with_scale(Vec3::new(width, length, 1.0))
        };
        *visibility = Visibility::Visible;
    }
}

/// The heights last asked of the host for the height labels: the
/// world and the first cell they are of, and the heights.
type HeightsAsked = Option<((u64, (u32, u32)), Vec<Height>)>;

/// Writes every cell's height on it, as the world run is generated,
/// once the cells are large enough on the screen to read it and few
/// enough for the labels there are.
#[allow(clippy::too_many_arguments)]
pub fn heights(shown: Res<Shown>, sprites: Res<Sprites>, seen: Res<Seen>, link: Res<Link>, mut asked: Local<HeightsAsked>, camera: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut labels: Query<(&HeightLabel, &mut Text2d, &mut Transform, &mut Visibility)>) {
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let [(first_x, last_x), (first_y, last_y)] = sprites.viewport_cells(transform, view.scale, &window);
    let (first, last) = ((first_x, first_y), (last_x, last_y));
    let readable = shown.heights && 1.0 / view.scale >= HEIGHT_FROM && last.0 - first.0 < HEIGHT_LABELS.0 && last.1 - first.1 < HEIGHT_LABELS.1;
    let size = view.scale * (1.0 / view.scale / HEIGHT_WIDTH).min(1.0);
    let world = seen.frame.as_ref().filter(|_| readable).map(|frame| frame.world);
    // The heights of the cells the labels may stand on, asked of the host once for a view: again only when it has moved, or the world is another.
    if let Some(world) = world
        && asked.as_ref().is_none_or(|(of, _)| *of != (world, first))
    {
        *asked = link.host.terrain().heights(HeightsAsk { world, first, size: HEIGHT_LABELS, skipped: Vec::new() }).map(|heights| ((world, first), heights));
    }
    let of = world.and(asked.as_ref()).filter(|(of, _)| of.1 == first).map(|(_, heights)| heights);
    for (label, mut text, mut transform, mut visibility) in &mut labels {
        // The cell the camera shows that is the label's: the first at or past the view's first whose place round the grid is its slot.
        let round = |first: u32, slot: u32, labels: u32| first + (slot + labels - first % labels) % labels;
        let (x, y) = (round(first.0, label.slot.0, HEIGHT_LABELS.0), round(first.1, label.slot.1, HEIGHT_LABELS.1));
        let Some(heights) = of.filter(|_| x <= last.0 && y <= last.1) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let height = heights[((y - first.1) * HEIGHT_LABELS.0 + x - first.0) as usize].to_string();
        if text.0 != height {
            text.0 = height;
        }
        *transform = Transform::from_xyz(sprites.plane(x, 0) + 0.5, -(sprites.plane(y, 1) + 0.5), 4.0).with_scale(Vec3::splat(size));
        *visibility = Visibility::Visible;
    }
}

/// Labels the superchunks and chunks the camera shows whose boundaries are
/// shown, once they are large enough on the screen: each one's Morton
/// index and its `(x, y)`, in its top left corner -- a chunk's a line
/// below, clear of its superchunk's.
pub fn labels(shown: Res<Shown>, sprites: Res<Sprites>, camera: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut labels: Query<(&mut Text2d, &mut Transform, &mut Visibility), With<Label>>) {
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    // The chunks the camera shows, counted from the world's top left: every label is at a chunk's corner.
    let [(first_x, last_x), (first_y, last_y)] = sprites.viewport_cells(transform, view.scale, &window).map(|(first, last)| (first / CHUNK_SIDE as u32, last / CHUNK_SIDE as u32));
    // How large a label of something `cells` across is drawn, of its full size: none if there is no room to read it.
    let fitted = |cells: usize| Some(cells as f32 / view.scale).filter(|&room| room >= LABELLED_FROM).map(|room| (room / LABEL_WIDTH).min(1.0));
    let (superchunk_size, chunk_size) = (fitted(SPRITE_SIDE as usize).filter(|_| shown.superchunks), fitted(CHUNK_SIDE).filter(|_| shown.chunks));
    let mut wanted = Vec::new();
    for y in first_y..=last_y {
        for x in first_x..=last_x {
            let (within_x, within_y) = (x % SUPERCHUNK_SIDE as u32, y % SUPERCHUNK_SIDE as u32);
            let superchunk = SuperchunkIndex::from_cartesian(x / SUPERCHUNK_SIDE as u32, y / SUPERCHUNK_SIDE as u32);
            let (superchunk_x, superchunk_y) = superchunk.cartesian();
            let corner = Vec2::new(sprites.plane(x * CHUNK_SIDE as u32, 0), -sprites.plane(y * CHUNK_SIDE as u32, 1));
            if let Some(size) = superchunk_size.filter(|_| (within_x, within_y) == (0, 0)) {
                wanted.push((corner, 0.0, size, format!("superchunk {:011x} ({superchunk_x}, {superchunk_y})", superchunk.0)));
            }
            if let Some(size) = chunk_size {
                let place = place_from_cartesian(within_x * CHUNK_SIDE as u32, within_y * CHUNK_SIDE as u32) / CELLS_IN_CHUNK;
                wanted.push((corner, superchunk_size.unwrap_or(0.0), size, format!("chunk {:012x} ({x}, {y})", ChunkIndex::of(superchunk, place).0)));
            }
        }
    }
    let mut wanted = wanted.into_iter();
    for (mut text, mut transform, mut visibility) in &mut labels {
        let Some((corner, lines_above, size, label)) = wanted.next() else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if text.0 != label {
            text.0 = label;
        }
        // As large on the screen however near the view is, and a little in from the corner.
        let inset = Vec2::new(LABEL_LINE / 4.0, -LABEL_LINE / 4.0 - lines_above * LABEL_LINE) * view.scale;
        *transform = Transform::from_translation((corner + inset).extend(4.0)).with_scale(Vec3::splat(view.scale * size));
        *visibility = Visibility::Visible;
    }
}
