//! The base game's types registered in a world's registry, in their
//! order, each with its library and hooks ([`register`]): every rule's
//! code its library, hooked to run every tick, and the world
//! generation's, hooked to generate the world -- each made with the IDs
//! the world gave the types it works on, as they were registered. What
//! the program's initialization hands the server and the client
//! ([`register_types`]): neither names a type of the base game, and
//! the registry is made for each world.

use crate::base_types::{BaseTypes, SheepState, GRASS_RULE, SHEEP_RULE, TREES_RULE, WEATHER_RULE, WEATHER_TILE_SCALE};
use crate::libraries::collision::Collision;
use protocol::type_registry::{Bits16, Bits32, Bits4, Bits8, Hook, RegisteredType, Registration, RegistryRefusal, TypeRegistry};
use std::sync::Arc;

/// The base game's types registered in `type_registry`, in their order:
/// an attribute before the type of entity it is the default of, a
/// plane or a type of entity before its rules, what a type depends on
/// before it. The rules tick in it. The types as the world gave them
/// their IDs; or the first refused.
pub fn register(type_registry: &mut TypeRegistry) -> Result<BaseTypes, String> {
    let layer = |registered_type: RegisteredType| registered_type.layer_type().expect("a layer");
    let grass = layer(registered(type_registry, Registration::layer("grass"))?);
    let tree = layer(registered(type_registry, Registration::layer("tree"))?);
    let tree_stage = registered(type_registry, Registration::wide_plane::<Bits4>("tree stage"))?.wide_plane().expect("four bits a cell");
    let wall_east = layer(registered(type_registry, Registration::layer("wall east"))?);
    let wall_south = layer(registered(type_registry, Registration::layer("wall south"))?);
    // The collision plane's library works on the plane itself: made of its ID.
    let collision = type_registry.register_with_own_library(Registration::layer("collision"), |registered_type| Arc::new(Collision { plane: layer(registered_type.clone()) })).map_err(refused("collision"))?;
    let collision = layer(collision);
    let wall_north = layer(registered(type_registry, Registration::layer("wall north"))?);
    let wall_west = layer(registered(type_registry, Registration::layer("wall west"))?);
    let sheep_state = registered(type_registry, Registration::attribute::<SheepState>("sheep state"))?.attribute().expect("laid out as a sheep's state");
    let sheep = registered(type_registry, Registration::entity("sheep", Some("sheep state")).depending_on(&["collision"]).with_hooks(&[Hook::KeepsTheWorldHot]))?.entity_type().expect("a type of entity");
    let wet = layer(registered(type_registry, Registration::layer("wet"))?);
    let cloud = registered(type_registry, Registration::wide_plane_of_tiles::<Bits4>("cloud", WEATHER_TILE_SCALE))?.wide_plane().expect("four bits a tile");
    let climate = registered(type_registry, Registration::wide_plane_of_tiles::<Bits32>("climate", WEATHER_TILE_SCALE))?.wide_plane().expect("thirty-two bits a tile");
    let elevation = registered(type_registry, Registration::wide_plane_of_tiles::<Bits8>("elevation", WEATHER_TILE_SCALE))?.wide_plane().expect("eight bits a tile");
    let height = registered(type_registry, Registration::wide_plane::<Bits16>("height"))?.wide_plane().expect("sixteen bits a cell");
    let water_depth = registered(type_registry, Registration::wide_plane::<Bits16>("water depth"))?.wide_plane().expect("sixteen bits a cell");
    let base_types = BaseTypes { grass, tree, tree_stage, wall_east, wall_south, collision, wall_north, wall_west, sheep_state, sheep, wet, cloud, climate, elevation, height, water_depth };
    // Each rule's code, and the world generation's, made with the types it works on.
    let rule_hooks = [Hook::RunsEveryTick];
    registered(type_registry, Registration::sca_rule(GRASS_RULE, "grass").with_library(Arc::new(crate::sca_rules::grass::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::sca_rule(TREES_RULE, "tree").with_library(Arc::new(crate::sca_rules::trees::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::entity_rule(SHEEP_RULE, "sheep").with_library(Arc::new(crate::entity_rules::sheep::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::sca_rule(WEATHER_RULE, "climate").with_library(Arc::new(crate::sca_rules::weather::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::worldgen("world generation").depending_on(&["sheep", "collision"]).with_library(Arc::new(crate::worldgen::world_generation::code(base_types))).with_hooks(&[Hook::GeneratesTheWorld]))?;
    Ok(base_types)
}

/// `registration`'s type registered in `type_registry`, with its ID; or why
/// it was refused.
fn registered(type_registry: &mut TypeRegistry, registration: Registration) -> Result<RegisteredType, String> {
    let type_name = registration.name.clone();
    type_registry.register(registration).map_err(refused(&type_name))
}

/// Why the base game's type named `type_name` was refused, of the refusal.
fn refused(type_name: &str) -> impl FnOnce(RegistryRefusal) -> String {
    move |refusal| format!("the base game's type {type_name}: {refusal:?}")
}

/// The base game's types registered in `type_registry` ([`register`]):
/// what the program's initialization hands the server and the client
/// (`protocol::world_types::GameTypes`), each world's registry made and
/// frozen with it.
pub fn register_types(type_registry: &mut TypeRegistry) -> Result<(), String> {
    register(type_registry).map(drop)
}
