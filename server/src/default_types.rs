//! The base game's types registered, each with its library and hooks
//! ([`new_type_registry_with_default_types`]): every rule's code its
//! library, hooked to run every tick, and the world generation's,
//! hooked to generate the world. What every world made new has for
//! now, a world without them left for later. Here until the root
//! program builds the registry and hands it to the server.

use base_types::{CLIMATE, CLOUD, COLLISION, ELEVATION, GRASS, GRASS_RULE, RULE_OF_GRASS, RULE_OF_SHEEP, RULE_OF_TREES, RULE_OF_WEATHER, SHEEP, SHEEP_RULE, SHEEP_STATE, TREE, TREES_RULE, TREE_STAGE, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST, WEATHER_RULE, WET, WORLD_GENERATION};
use type_registry::{Hook, RegisteredType, Registration, TypeRegistry};

/// A registry of the base game's types, in the order they are
/// registered: an attribute before the type of entity it is the
/// default of, a plane or a type of entity before its rules, what a
/// type depends on before it. The rules tick in it.
pub fn new_type_registry_with_default_types() -> TypeRegistry {
    let mut type_registry = TypeRegistry::default();
    let default_types: [Registration; 19] = [
        RegisteredType::layer("grass", GRASS).into(),
        RegisteredType::layer("tree", TREE).into(),
        RegisteredType::wide_plane("tree stage", TREE_STAGE).into(),
        RegisteredType::layer("wall east", WALL_EAST).into(),
        RegisteredType::layer("wall south", WALL_SOUTH).into(),
        RegisteredType::layer("collision", COLLISION).with_library(&libraries::collision::COLLISION),
        RegisteredType::layer("wall north", WALL_NORTH).into(),
        RegisteredType::layer("wall west", WALL_WEST).into(),
        RegisteredType::attribute("sheep state", SHEEP_STATE).into(),
        RegisteredType::entity("sheep", SHEEP, Some(SHEEP_STATE)).depending_on(&["collision"]).with_hooks(&[Hook::KeepsTheWorldHot]),
        RegisteredType::layer("wet", WET).into(),
        RegisteredType::wide_plane("cloud", CLOUD).into(),
        RegisteredType::wide_plane("climate", CLIMATE).into(),
        RegisteredType::wide_plane("elevation", ELEVATION).into(),
        RegisteredType::ScaRule { name: GRASS_RULE.to_string(), rule: RULE_OF_GRASS, master: GRASS }.with_library(&sca_rules::grass::CODE).with_hooks(&[Hook::RunsEveryTick]),
        RegisteredType::ScaRule { name: TREES_RULE.to_string(), rule: RULE_OF_TREES, master: TREE }.with_library(&sca_rules::trees::CODE).with_hooks(&[Hook::RunsEveryTick]),
        RegisteredType::EntityRule { name: SHEEP_RULE.to_string(), rule: RULE_OF_SHEEP, master: SHEEP }.with_library(&entity_rules::sheep::CODE).with_hooks(&[Hook::RunsEveryTick]),
        RegisteredType::ScaRule { name: WEATHER_RULE.to_string(), rule: RULE_OF_WEATHER, master: CLIMATE.layer_type() }.with_library(&sca_rules::weather::CODE).with_hooks(&[Hook::RunsEveryTick]),
        RegisteredType::Worldgen { name: "world generation".to_string(), worldgen: WORLD_GENERATION }.depending_on(&["sheep", "collision"]).with_library(&worldgen::world_generation::WORLD_GENERATION).with_hooks(&[Hook::GeneratesTheWorld]),
    ];
    for default_type in default_types {
        let type_name = default_type.registered_type.name().to_string();
        type_registry.register(default_type).unwrap_or_else(|refusal| panic!("the default type {type_name}: {refusal:?}"));
    }
    type_registry
}
