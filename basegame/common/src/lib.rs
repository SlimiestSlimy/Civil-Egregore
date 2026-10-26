//! The base game's common bundle: grass, a plane -- data, no code. What
//! it makes of grass is its own full name, a type of the game's: what
//! the game downcasts, to see one type is one type on both sides.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_names::{BundleName, TypeName};
use protocol::type_registry::{Export, TypeRegistryHandle};

/// The bundle set up: grass's full name, its one export.
#[unsafe(no_mangle)]
pub fn bundle_set_up(_registry: &TypeRegistryHandle<'_>) -> Result<Vec<(&'static str, Export)>, String> {
    let grass = TypeName { bundle_name: BundleName { mod_name: "basegame".to_string(), bundle: "common".to_string() }, type_name: "grass".to_string() };
    Ok(vec![("grass", Box::new(grass))])
}
