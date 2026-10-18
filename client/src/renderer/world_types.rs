//! The types of the world shown, as the window has them: its own
//! registry of the base game's types frozen with the world's IDs, as
//! the server sends them once, before the world's first frame
//! ([`base_types_of`]). For now the window draws
//! the base game's types by name of its own ([`BaseTypes`]), until the
//! types draw themselves.

use basegame::base_types::BaseTypes;
use protocol::type_registry::TypeRegistry;

/// The base game's types of the world whose types `world_types` names
/// (`protocol::type_registry::FrozenTypes::to_text`), each of the ID
/// the world gave it: none if the text is not a world's types, or
/// they are not the base game's.
pub fn base_types_of(world_types: &str) -> Option<BaseTypes> {
    let mut type_registry = TypeRegistry::for_saved_world(world_types).ok()?;
    let base_types = basegame::default_types::register(&mut type_registry).ok()?;
    // Frozen, to be checked as the server's was: every type the world has is registered.
    type_registry.freeze().ok().map(|_| base_types)
}
