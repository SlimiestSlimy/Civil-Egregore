//! How the base game is drawn: its render rules, registered whether or
//! not the client is built -- a world's types are the same headless or
//! windowed -- their code, Bevy's, only with the `client` feature
//! ([`drawn`]). A rule is drawn on its render layer, over the lower:
//! the ground, grass, trees, water, walls, sheep.

#[cfg(feature = "client")]
pub mod drawn;

use crate::base_types::BaseTypes;
use crate::default_types::registered;
use protocol::type_registry::{Registration, TypeRegistry};
#[cfg(feature = "client")]
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
/// types they draw as `base_types` has them: each one's code its
/// library, built with the client.
#[cfg_attr(not(feature = "client"), allow(unused_variables))]
pub fn register(type_registry: &mut TypeRegistry, base_types: BaseTypes) -> Result<(), String> {
    let ground = Registration::render_rule("ground drawn", "height", GROUND_LAYER);
    #[cfg(feature = "client")]
    let ground = ground.with_library(Arc::new(drawn::ground(base_types)));
    let grass = Registration::render_rule("grass drawn", "grass", GRASS_LAYER);
    #[cfg(feature = "client")]
    let grass = grass.with_library(Arc::new(drawn::grass(base_types)));
    let trees = Registration::render_rule("trees drawn", "tree", TREES_LAYER).depending_on(&["tree stage"]);
    #[cfg(feature = "client")]
    let trees = trees.with_library(Arc::new(drawn::trees(base_types)));
    let water = Registration::render_rule("water drawn", "wet", WATER_LAYER).depending_on(&["water depth"]);
    #[cfg(feature = "client")]
    let water = water.with_library(Arc::new(drawn::water(base_types)));
    let walls = Registration::render_rule("walls drawn", "wall east", WALLS_LAYER).depending_on(&["wall south", "wall west", "wall north"]);
    #[cfg(feature = "client")]
    let walls = walls.with_library(Arc::new(drawn::walls(base_types)));
    let sheep = Registration::render_rule("sheep drawn", "sheep", SHEEP_LAYER);
    #[cfg(feature = "client")]
    let sheep = sheep.with_library(Arc::new(drawn::sheep(base_types)));
    for render_rule in [ground, grass, trees, water, walls, sheep] {
        registered(type_registry, render_rule)?;
    }
    Ok(())
}
