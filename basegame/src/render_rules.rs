//! How the base game is drawn: its render rules, each with its code
//! ([`drawn`]). A rule is drawn on its render layer, over the lower:
//! the ground, grass, trees, water, walls, sheep.

pub mod drawn;

use crate::base_types::BaseTypes;
use crate::default_types::registered;
use protocol::type_registry::{Registration, TypeRegistry};
use std::sync::Arc;

/// The ground's render layer: under everything.
pub const GROUND_LAYER: u32 = 0;
/// Grass's, over the ground.
pub const GRASS_LAYER: u32 = 1;
/// Trees', over grass.
pub const TREES_LAYER: u32 = 2;
/// Water's, over what grows.
pub const WATER_LAYER: u32 = 3;
/// The walls', over water.
pub const WALLS_LAYER: u32 = 4;
/// The sheep's, over everything.
pub const SHEEP_LAYER: u32 = 5;

/// The base game's render rules registered in `type_registry`, with the
/// types they draw as `base_types` has them: each one's code its library.
pub fn register(type_registry: &mut TypeRegistry, base_types: BaseTypes) -> Result<(), String> {
    let render_rules = [
        Registration::render_rule("ground drawn", "height", GROUND_LAYER).with_library(Arc::new(drawn::ground(base_types))),
        Registration::render_rule("grass drawn", "grass", GRASS_LAYER).with_library(Arc::new(drawn::grass(base_types))),
        Registration::render_rule("trees drawn", "tree", TREES_LAYER).depending_on(&["tree stage"]).with_library(Arc::new(drawn::trees(base_types))),
        Registration::render_rule("water drawn", "wet", WATER_LAYER).depending_on(&["water depth"]).with_library(Arc::new(drawn::water(base_types))),
        Registration::render_rule("walls drawn", "wall east", WALLS_LAYER).depending_on(&["wall south", "wall west", "wall north"]).with_library(Arc::new(drawn::walls(base_types))),
        Registration::render_rule("sheep drawn", "sheep", SHEEP_LAYER).with_library(Arc::new(drawn::sheep(base_types))),
    ];
    for render_rule in render_rules {
        registered(type_registry, render_rule)?;
    }
    Ok(())
}
