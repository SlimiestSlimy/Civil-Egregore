//! Every tile's climate written on it, by its key: its humidity, its
//! temperature and its wind, as the last frame brought them
//! (`docs/renderer.md`, "The climate, written").

use crate::link::Seen;
use crate::overlays::Shown;
use crate::view::{Sprites, SPRITE_SIDE};
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
const CLIMATE_WIDTH: f32 = 110.0;
/// The ways a wind blows towards, named, in the order the type
/// registry has them (`type_registry::WAYS_OF_WIND`).
const WAYS_NAMED: [&str; 4] = ["north", "west", "south", "east"];

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
pub fn spawn(mut commands: Commands) {
    for slot in 0..CLIMATE_LABELS.0 * CLIMATE_LABELS.1 {
        commands.spawn((Text2d::new(""), Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, ClimateLabel { slot: (slot % CLIMATE_LABELS.0, slot / CLIMATE_LABELS.0) }));
    }
}

/// What is written on a tile of `climate`: its humidity, its
/// temperature, and the way its wind blows towards with its strength
/// -- or that it is calm.
fn written(climate: Climate) -> String {
    let wind = climate.wind.way().map_or("calm".to_string(), |way| format!("{} {}", WAYS_NAMED[way], climate.wind.strength()));
    format!("humid {}\nheat {}\n{wind}", climate.humidity, climate.temperature)
}

/// Writes every tile's climate on it, as the last frame of its
/// superchunk brought it, once the tiles are large enough on the
/// screen to read it and few enough for the labels there are.
pub fn climates(shown: Res<Shown>, sprites: Res<Sprites>, seen: Res<Seen>, camera: Single<(&Transform, &Projection), CameraOnly>, window: Single<&Window>, mut labels: Query<(&ClimateLabel, &mut Text2d, &mut Transform, &mut Visibility)>) {
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    // The tiles the camera shows, counted from the world's top left.
    let [(first_x, last_x), (first_y, last_y)] = sprites.viewport_cells(transform, view.scale, &window).map(|(first, last)| (first / TILE_SIDE, last / TILE_SIDE));
    let across = TILE_SIDE as f32 / view.scale;
    let readable = shown.climates && across >= CLIMATE_FROM && last_x - first_x < CLIMATE_LABELS.0 && last_y - first_y < CLIMATE_LABELS.1;
    let size = view.scale * (across / CLIMATE_WIDTH).min(1.0);
    for (label, mut text, mut transform, mut visibility) in &mut labels {
        // The tile the camera shows that is the label's: the first at or past the view's first whose place round the grid is its slot.
        let round = |first: u32, slot: u32, labels: u32| first + (slot + labels - first % labels) % labels;
        let (x, y) = (round(first_x, label.slot.0, CLIMATE_LABELS.0), round(first_y, label.slot.1, CLIMATE_LABELS.1));
        let (left, top) = (x * TILE_SIDE, y * TILE_SIDE);
        let side = SPRITE_SIDE as u32;
        let climate = Some(()).filter(|()| readable && x <= last_x && y <= last_y).and_then(|()| seen.climates.get(&(left / side, top / side))).and_then(|climates| climate_over(climates, place_from_cartesian(left % side, top % side) >> CLIMATE_TILE_SHIFT));
        let Some(climate) = climate else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let climate = written(climate);
        if text.0 != climate {
            text.0 = climate;
        }
        let middle = TILE_SIDE as f32 / 2.0;
        *transform = Transform::from_xyz(sprites.plane(left, 0) + middle, -(sprites.plane(top, 1) + middle), 4.0).with_scale(Vec3::splat(size));
        *visibility = Visibility::Visible;
    }
}
