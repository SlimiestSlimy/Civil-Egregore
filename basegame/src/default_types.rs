//! The base game's types registered in a world's registry, in their
//! order, each with its library and hooks ([`register`]): every rule's
//! code its library, hooked to run every tick, the world
//! generation's, hooked to generate the world, and the functions of it
//! a client may call -- each made with the IDs
//! the world gave the types it works on, as they were registered. What
//! the program's initialization hands the server and the client
//! ([`register_types`]): neither names a type of the base game, and
//! the registry is made for each world.

use crate::base_types::{BaseTypes, SheepState, GRASS_RULE, SHEEP_RULE, TREES_RULE};
use crate::libraries::collision::Collision;
use protocol::type_registry::{Hook, RegisteredType, Registration, RegistryRefusal, TypeRegistry};
use server::simulation::simulation_api::{Attribute, Bits16, Bits4, WidePlane};
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
    let tree_stage = WidePlane::registered(&registered(type_registry, WidePlane::<Bits4>::registration("tree stage"))?).expect("four bits a cell");
    let wall_east = layer(registered(type_registry, Registration::layer("wall east"))?);
    let wall_south = layer(registered(type_registry, Registration::layer("wall south"))?);
    // The collision plane's library works on the plane itself: made of its ID.
    let collision = type_registry.register_with_own_library(Registration::layer("collision"), |registered_type| Arc::new(Collision { plane: layer(registered_type.clone()) })).map_err(refused("collision"))?;
    let collision = layer(collision);
    let wall_north = layer(registered(type_registry, Registration::layer("wall north"))?);
    let wall_west = layer(registered(type_registry, Registration::layer("wall west"))?);
    let sheep_state = Attribute::registered(&registered(type_registry, Attribute::<SheepState>::registration("sheep state"))?).expect("laid out as a sheep's state");
    let sheep = registered(type_registry, Registration::entity("sheep", Some("sheep state")).depending_on(&["collision"]).with_hooks(&[Hook::KeepsTheWorldHot]))?.entity_type().expect("a type of entity");
    let wet = layer(registered(type_registry, Registration::layer("wet"))?);
    let height = WidePlane::registered(&registered(type_registry, WidePlane::<Bits16>::registration("height"))?).expect("sixteen bits a cell");
    let water_depth = WidePlane::registered(&registered(type_registry, WidePlane::<Bits16>::registration("water depth"))?).expect("sixteen bits a cell");
    let base_types = BaseTypes { grass, tree, tree_stage, wall_east, wall_south, collision, wall_north, wall_west, sheep_state, sheep, wet, height, water_depth };
    // Each rule's code, and the world generation's, made with the types it works on.
    let rule_hooks = [Hook::RunsEveryTick];
    registered(type_registry, Registration::sca_rule(GRASS_RULE, "grass").with_library(Arc::new(crate::sca_rules::grass::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::sca_rule(TREES_RULE, "tree").with_library(Arc::new(crate::sca_rules::trees::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::entity_rule(SHEEP_RULE, "sheep").with_library(Arc::new(crate::entity_rules::sheep::code(base_types))).with_hooks(&rule_hooks))?;
    registered(type_registry, Registration::worldgen("world generation").depending_on(&["sheep", "collision"]).with_library(Arc::new(crate::worldgen::world_generation::code(base_types))).with_hooks(&[Hook::GeneratesTheWorld]))?;
    // What a client may call of the world generation.
    for call_registration in crate::worldgen::calls::call_registrations("world generation") {
        registered(type_registry, call_registration)?;
    }
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
/// (`protocol::world_types::RegisterTypes`), each world's registry made and
/// frozen with it.
pub fn register_types(type_registry: &mut TypeRegistry) -> Result<(), String> {
    register(type_registry).map(drop)
}
