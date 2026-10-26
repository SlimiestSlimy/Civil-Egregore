//! The base game's server bundle: no type of its own yet but `grass_seen`,
//! a sca proc -- what it makes of it is what it took of the common
//! bundle, grass's export downcast once at set up: the first thing one
//! bundle takes of another.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_names::TypeName;
use protocol::type_registry::{Export, TypeRegistryHandle};

/// What `grass_seen` is made of: grass's full name, as the common bundle
/// exported it, and grass's ID.
pub struct GrassSeen {
    /// Grass's full name, taken from the common bundle's export.
    pub grass_name: TypeName,
    /// Grass's ID.
    pub grass_id: u64,
}

/// The bundle set up: grass's export taken from the common bundle,
/// downcast to what it is, and kept in `grass_seen`.
#[unsafe(no_mangle)]
pub fn bundle_set_up(registry: &TypeRegistryHandle<'_>) -> Result<Vec<(&'static str, Export)>, String> {
    const GRASS: &str = "basegame>common>grass";
    let grass_export = registry.code_of(GRASS).ok_or_else(|| format!("{GRASS}: nothing made of it"))?;
    let grass_name = grass_export.downcast_ref::<TypeName>().ok_or_else(|| format!("{GRASS}: not a TypeName"))?.clone();
    let grass_id = registry.type_id_of(GRASS).ok_or_else(|| format!("{GRASS}: no such type"))?;
    Ok(vec![("grass_seen", Box::new(GrassSeen { grass_name, grass_id }))])
}
