//! The first steps of making or loading a world, before the server
//! starts it or the client draws it: its types registered and frozen
//! ([`FrozenTypeRegistry`]). A new world's IDs are handed out as its game's
//! types are registered; a saved world's are those its types file gave
//! each name. Nothing before this has a type: the registry is made for
//! a world, not for the program; and a type's name is for registering
//! it: frozen, it is known by its ID alone.

use crate::type_registry::{FrozenTypeRegistry, TypeRegistry};
use std::path::Path;

/// What registers a game's types in a registry, in their order -- each
/// library made with the IDs the types registered before it were
/// given: what every world is made of. Or why one was refused.
pub type RegisterTypes = fn(&mut TypeRegistry) -> Result<(), String>;

/// The file a world's save keeps its types in, in its folder.
pub const TYPES_FILE: &str = "types.csv";

/// The types of a new world of `register_types`, each given the next ID.
pub fn of_a_new_world(register_types: RegisterTypes) -> Result<FrozenTypeRegistry, String> {
    registered(TypeRegistry::for_new_world(), register_types)
}

/// The types of the world whose types `types_text` names, of
/// `register_types`: each given the ID the text gives its name, any the
/// text does not name the next; or what is wrong.
pub fn of_a_saved_world(types_text: &str, register_types: RegisterTypes) -> Result<FrozenTypeRegistry, String> {
    registered(TypeRegistry::for_saved_world(types_text)?, register_types)
}

/// The types of the world saved in `folder`, of `register_types`, as its
/// types file gives them ([`of_a_saved_world`]).
pub fn saved_in(folder: &Path, register_types: RegisterTypes) -> Result<FrozenTypeRegistry, String> {
    let path = folder.join(TYPES_FILE);
    let types_text = std::fs::read_to_string(&path).map_err(|read_error| format!("{}: {read_error}", path.display()))?;
    of_a_saved_world(&types_text, register_types).map_err(|what_is_wrong| format!("{}: {what_is_wrong}", path.display()))
}

/// `register_types` registered in `type_registry`, and frozen.
fn registered(mut type_registry: TypeRegistry, register_types: RegisterTypes) -> Result<FrozenTypeRegistry, String> {
    register_types(&mut type_registry)?;
    type_registry.freeze()
}
