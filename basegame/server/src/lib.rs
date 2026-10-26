//! The base game's server mod: no type of its own yet but
//! `grass_tick`, a sca proc sampled on grass -- what it makes of it is
//! what it took of the common mod, grass's export downcast once at
//! set up: the first thing one mod takes of another.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_names::QualifiedTypeName;
use protocol::type_registry::{TypeCode, TypeRegistryHandle};
use server::sca_proc::ScaProc;
use server::server_handle::ServerHandle;
use utilities::coordinates::CellIndex;

/// What `grass_tick` is made of: grass's qualified name, as the common mod
/// exported it, and grass's ID.
pub struct GrassTick {
    /// Grass's qualified name, taken from the common mod's export.
    pub grass_name: QualifiedTypeName,
    /// Grass's ID.
    pub grass_id: u64,
}

impl ScaProc for GrassTick {
    /// Nothing yet: the server's handle has nothing to give.
    fn call(&self, _server: &mut ServerHandle, _sampled_plane_type_id: u64, _tile_corner: CellIndex) {}
}

/// `grass_tick` set up: grass's export taken from the common mod,
/// downcast to what it is, and kept; a sca proc to the server.
#[unsafe(no_mangle)]
pub fn grass_tick_set_up(registry: &TypeRegistryHandle<'_>) -> Result<TypeCode, String> {
    const GRASS: &str = "basegame_common>grass";
    let grass_export = registry.code_of(GRASS).ok_or_else(|| format!("{GRASS}: nothing made of it"))?;
    let grass_name = grass_export.downcast_ref::<QualifiedTypeName>().ok_or_else(|| format!("{GRASS}: not a QualifiedTypeName"))?.clone();
    let grass_id = registry.type_id_of(GRASS).ok_or_else(|| format!("{GRASS}: no such type"))?;
    Ok(Box::new(Box::new(GrassTick { grass_name, grass_id }) as Box<dyn ScaProc>))
}
