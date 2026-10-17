//! Every tile's climate written on it, by its key: its humidity, its
//! temperature and its wind, as the last frame brought them
//! (`docs/renderer.md`, "The climate, written").

use crate::renderer::link::LastFrameSeen;
use crate::renderer::overlays::OverlaysShown;
use crate::renderer::view::{SuperchunkSprites, SPRITE_SIDE};
use bevy::prelude::*;
use coordinates::place_from_cartesian;
use server::host::frame::{climate_over, Climate, CLIMATE_TILE_SHIFT};

/// Cells along a side of a tile of the climate.
const TILE_SIDE: u32 = 1 << (CLIMATE_TILE_SHIFT / 2);
/// Climate labels across and down: the most tiles the camera shows
/// that are written on.
const CLIMATE_LABELS: (u32, u32) = (48, 28);
/// Screen pixels a tile is across before its climate is written on it.
const CLIMATE_FROM: f32 = 40.0;
/// Screen pixels a climate is across at its full size: under that
/// much room it is written smaller, to fit.
const CLIMATE_WIDTH: f32 = 120.0;

/// A tile's climate, written on it: one of a grid of them, each given
/// to the tile the camera shows whose place is its own, counted round
/// the grid -- so a tile keeps its text while the view moves.
#[derive(Component)]
pub struct ClimateLabel {
    /// Where it is in the grid.
    slot: (u32, u32),
}

/// The camera, and no label: both have a place.
type CameraOnly = (With<Camera2d>, Without<ClimateLabel>);

/// The labels, hidden until asked for.
pub fn spawn_climate_labels(mut bevy_commands: Commands) {
    for slot in 0..CLIMATE_LABELS.0 * CLIMATE_LABELS.1 {
        bevy_commands.spawn((Text2d::new(""), Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, ClimateLabel { slot: (slot % CLIMATE_LABELS.0, slot / CLIMATE_LABELS.0) }));
    }
}

/// What is written on a tile of `climate`: its humidity, its
/// temperature, its pressure, and its wind -- how strongly it blows
/// east and south, under 0 west and north, or that it is calm.
fn climate_written(climate: Climate) -> String {
    let wind = match climate.wind.east_south_vector() {
        (0, 0) => "calm".to_string(),
        (wind_east, wind_south) => format!("wind {wind_east:+} {wind_south:+}"),
    };
    format!("humid {}\nheat {}\npress {}\n{wind}", climate.humidity, climate.temperature, climate.pressure)
}

/// Writes every tile's climate on it, as the last frame of its
/// superchunk brought it, once the tiles are large enough on the
/// screen to read it and few enough for the labels there are.
pub fn climates(overlays_shown: Res<OverlaysShown>, superchunk_sprites: Res<SuperchunkSprites>, last_seen: Res<LastFrameSeen>, camera_parts: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut tile_labels: Query<(&ClimateLabel, &mut Text2d, &mut Transform, &mut Visibility)>) {
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    // The tiles the camera shows, counted from the world's top left.
    let [(first_x, last_x), (first_y, last_y)] = superchunk_sprites.viewport_cells(camera_transform, orthographic_view.scale, &window).map(|(first_cell_along, last_cell_along)| (first_cell_along / TILE_SIDE, last_cell_along / TILE_SIDE));
    let tile_pixels_across = TILE_SIDE as f32 / orthographic_view.scale;
    let climates_readable = overlays_shown.climates && tile_pixels_across >= CLIMATE_FROM && last_x - first_x < CLIMATE_LABELS.0 && last_y - first_y < CLIMATE_LABELS.1;
    let label_scale = orthographic_view.scale * (tile_pixels_across / CLIMATE_WIDTH).min(1.0);
    for (climate_label, mut label_text, mut label_transform, mut label_visibility) in &mut tile_labels {
        // The tile the camera shows that is the label's: the first at or past the view's first whose place round the grid is its slot.
        let round_to_slot = |first_along: u32, slot: u32, labels_along: u32| first_along + (slot + labels_along - first_along % labels_along) % labels_along;
        let (x, y) = (round_to_slot(first_x, climate_label.slot.0, CLIMATE_LABELS.0), round_to_slot(first_y, climate_label.slot.1, CLIMATE_LABELS.1));
        let (left_cell_column, top_cell_row) = (x * TILE_SIDE, y * TILE_SIDE);
        let superchunk_side_cells = SPRITE_SIDE as u32;
        let climate = Some(()).filter(|()| climates_readable && x <= last_x && y <= last_y).and_then(|()| last_seen.climates.get(&(left_cell_column / superchunk_side_cells, top_cell_row / superchunk_side_cells))).and_then(|climates| climate_over(climates, place_from_cartesian(left_cell_column % superchunk_side_cells, top_cell_row % superchunk_side_cells) >> CLIMATE_TILE_SHIFT));
        let Some(climate) = climate else {
            *label_visibility = Visibility::Hidden;
            continue;
        };
        let climate = climate_written(climate);
        if label_text.0 != climate {
            label_text.0 = climate;
        }
        let half_tile = TILE_SIDE as f32 / 2.0;
        *label_transform = Transform::from_xyz(superchunk_sprites.translation_of_cell(left_cell_column, 0) + half_tile, -(superchunk_sprites.translation_of_cell(top_cell_row, 1) + half_tile), 4.0).with_scale(Vec3::splat(label_scale));
        *label_visibility = Visibility::Visible;
    }
}
