//! The render rules' code: each the client's loop over what it draws,
//! with the colour of one drawn cell -- of many cells, seen from far:
//! a plane's share set, or its first cell's number, stands for them.

use crate::base_types::{BaseTypes, OLDEST_TREE_STAGE};
use bevy::color::{Alpha, Color, Mix};
use client::client_api::rendering::{on_each_drawn_cell, on_each_entity, on_each_set_cell, RenderCode};

/// The ground at its lowest shown: the ocean's floor, dark.
const LOWEST_GROUND: u32 = 256;
/// The heights from the lowest to the brightest ground.
const GROUND_HEIGHTS: u32 = 640;
/// The ground at its lowest.
const LOW_GROUND: Color = Color::srgb_u8(58, 44, 30);
/// The ground at its highest.
const HIGH_GROUND: Color = Color::srgb_u8(196, 178, 150);
/// Grass.
const GRASS: Color = Color::srgb_u8(84, 156, 58);
/// A young tree.
const YOUNG_TREE: Color = Color::srgb_u8(70, 130, 50);
/// An old tree.
const OLD_TREE: Color = Color::srgb_u8(18, 62, 24);
/// Shallow water.
const SHALLOW_WATER: Color = Color::srgba_u8(70, 140, 200, 200);
/// Water at the deep water's depth, and deeper.
const DEEP_WATER: Color = Color::srgba_u8(14, 34, 92, 250);
/// The depth water is darkest from.
const DARKEST_DEPTH: u32 = 128;
/// A wall.
const WALL: Color = Color::srgba_u8(40, 36, 32, 220);
/// A sheep.
const SHEEP: Color = Color::srgb_u8(250, 250, 244);

/// The ground: every cell, by its height, the higher the brighter.
pub fn ground(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| {
        on_each_drawn_cell(turn, |turn, drawn| {
            let height = turn.value(base_types.height.layer_type(), drawn);
            let brightness = height.saturating_sub(LOWEST_GROUND).min(GROUND_HEIGHTS) as f32 / GROUND_HEIGHTS as f32;
            Some(LOW_GROUND.mix(&HIGH_GROUND, brightness))
        });
    })
}

/// Grass: as thick as the share of the cells it covers.
pub fn grass(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| on_each_set_cell(turn, base_types.grass, |turn, drawn| Some(GRASS.with_alpha(0.35 + 0.65 * turn.share_set(base_types.grass, drawn)))))
}

/// Trees: the older, the darker.
pub fn trees(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| {
        on_each_set_cell(turn, base_types.tree, |turn, drawn| {
            let stage = turn.value(base_types.tree_stage.layer_type(), drawn);
            Some(YOUNG_TREE.mix(&OLD_TREE, stage as f32 / OLDEST_TREE_STAGE as f32))
        });
    })
}

/// Water: the deeper, the darker.
pub fn water(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| {
        on_each_set_cell(turn, base_types.wet, |turn, drawn| {
            let depth = turn.value(base_types.water_depth.layer_type(), drawn);
            Some(SHALLOW_WATER.mix(&DEEP_WATER, depth.min(DARKEST_DEPTH) as f32 / DARKEST_DEPTH as f32))
        });
    })
}

/// Walls: a cell with a wall any way.
pub fn walls(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| {
        for wall in base_types.walls() {
            on_each_set_cell(turn, wall, |_, _| Some(WALL));
        }
    })
}

/// Sheep: each one where it stands.
pub fn sheep(base_types: BaseTypes) -> RenderCode {
    RenderCode::new(move |turn, _| on_each_entity(turn, base_types.sheep, |_, _| Some(SHEEP)))
}
