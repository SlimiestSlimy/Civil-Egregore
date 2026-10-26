//! The base game's server mod: no type of its own yet but
//! `grass_tick`, a sca proc sampled on grass -- what it makes of it is
//! what it took of the common mod, grass's export downcast once at
//! set up: the first thing one mod takes of another.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_names::TypeName;
use protocol::type_registry::{Export, TypeRegistryHandle};

/// What `grass_tick` is made of: grass's full name, as the common mod
/// exported it, and grass's ID.
pub struct GrassTick {
    /// Grass's full name, taken from the common mod's export.
    pub grass_name: TypeName,
    /// Grass's ID.
    pub grass_id: u64,
}

/// `grass_tick` set up: grass's export taken from the common mod,
/// downcast to what it is, and kept.
#[unsafe(no_mangle)]
pub fn grass_tick_set_up(registry: &TypeRegistryHandle<'_>) -> Result<Export, String> {
    const GRASS: &str = "basegame_common>grass";
    let grass_export = registry.code_of(GRASS).ok_or_else(|| format!("{GRASS}: nothing made of it"))?;
    let grass_name = grass_export.downcast_ref::<TypeName>().ok_or_else(|| format!("{GRASS}: not a TypeName"))?.clone();
    let grass_id = registry.type_id_of(GRASS).ok_or_else(|| format!("{GRASS}: no such type"))?;
    Ok(Box::new(GrassTick { grass_name, grass_id }))
}
