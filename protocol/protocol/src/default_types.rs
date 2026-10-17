//! The base game's types registered, each rule with its code
//! ([`new_type_registry_with_default_types`]): what every world made new
//! has for now, a world without them left for later. The one place
//! outside the base game that depends on it.

use simulation::simulation_api::RuleCode;
use base_types::{CLIMATE, CLOUD, COLLISION, ELEVATION, GRASS, GRASS_RULE, RULE_OF_GRASS, RULE_OF_SHEEP, RULE_OF_TREES, RULE_OF_WEATHER, SHEEP, SHEEP_RULE, SHEEP_STATE, TREE, TREES_RULE, TREE_STAGE, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST, WEATHER_RULE, WET};
use type_registry::{RegisteredType, Registration, TypeRegistry};

/// A registry of the base game's types, in the order they are
/// registered: an attribute before the type of entity it is the
/// default of, a plane or a type of entity before its rules. The rules
/// tick in it.
pub fn new_type_registry_with_default_types() -> TypeRegistry<RuleCode> {
    let mut type_registry = TypeRegistry::default();
    let default_types: [Registration<RuleCode>; 18] = [
        RegisteredType::layer("grass", GRASS).into(),
        RegisteredType::layer("tree", TREE).into(),
        RegisteredType::wide_plane("tree stage", TREE_STAGE).into(),
        RegisteredType::layer("wall east", WALL_EAST).into(),
        RegisteredType::layer("wall south", WALL_SOUTH).into(),
        RegisteredType::layer("collision", COLLISION).with_library(&libraries::collision::COLLISION),
        RegisteredType::layer("wall north", WALL_NORTH).into(),
        RegisteredType::layer("wall west", WALL_WEST).into(),
        RegisteredType::attribute("sheep state", SHEEP_STATE).into(),
        RegisteredType::entity("sheep", SHEEP, Some(SHEEP_STATE)).depending_on(&["collision"]),
        RegisteredType::layer("wet", WET).into(),
        RegisteredType::wide_plane("cloud", CLOUD).into(),
        RegisteredType::wide_plane("climate", CLIMATE).into(),
        RegisteredType::wide_plane("elevation", ELEVATION).into(),
        RegisteredType::ScaRule { name: GRASS_RULE.to_string(), rule: RULE_OF_GRASS, master: GRASS, code: sca_rules::grass::CODE }.into(),
        RegisteredType::ScaRule { name: TREES_RULE.to_string(), rule: RULE_OF_TREES, master: TREE, code: sca_rules::trees::CODE }.into(),
        RegisteredType::EntityRule { name: SHEEP_RULE.to_string(), rule: RULE_OF_SHEEP, master: SHEEP, code: entity_rules::sheep::CODE }.into(),
        RegisteredType::ScaRule { name: WEATHER_RULE.to_string(), rule: RULE_OF_WEATHER, master: CLIMATE.layer_type(), code: sca_rules::weather::CODE }.into(),
    ];
    for default_type in default_types {
        let type_name = default_type.registered_type.name().to_string();
        type_registry.register(default_type).unwrap_or_else(|refusal| panic!("the default type {type_name}: {refusal:?}"));
    }
    type_registry
}
