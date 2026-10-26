//! The base game's common mod: grass, a plane -- data, no code. What
//! its set up makes of grass is its own qualified name, a type of the game's:
//! what the game downcasts, to see one type is one type on both sides.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_names::QualifiedTypeName;
use protocol::type_registry::{TypeCode, TypeRegistryHandle};

/// Grass set up: its qualified name, what it gives its dependents.
#[unsafe(no_mangle)]
pub fn grass_set_up(_registry: &TypeRegistryHandle<'_>) -> Result<TypeCode, String> {
    Ok(Box::new(QualifiedTypeName { mod_name: "basegame_common".to_string(), type_name: "grass".to_string() }))
}
