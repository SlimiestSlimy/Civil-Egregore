//! The base game's server mod: no type of its own yet but
//! `grass_tick`, a sca proc sampled on grass, holding grass's type ID
//! taken from the registry at set up.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::type_registry::TypeRegistry;
use server::sca_proc::{ScaProc, ScaProcSetUp};
use server::server_handle::ServerHandle;
use utilities::coordinates::CellIndex;

/// What `grass_tick` is made of: grass's type ID.
pub struct GrassTick {
    /// Grass's type ID.
    pub grass_type_id: u64,
}

impl ScaProc for GrassTick {
    /// Nothing yet: the server's handle has nothing to give.
    fn call(&self, _server: &mut ServerHandle, _sampled_plane_type_id: u64, _tile_corner: CellIndex) {}
}

/// `grass_tick` set up: grass's type ID, taken by its name.
pub fn grass_tick_set_up(type_registry: &TypeRegistry) -> Box<dyn ScaProc> {
    let grass_type_id = type_registry.type_id_of("basegame_common>grass").expect("grass, of the common mod this one depends on");
    Box::new(GrassTick { grass_type_id })
}

/// `grass_tick`'s set up, as its premanifest names it.
#[unsafe(no_mangle)]
pub static GRASS_TICK_SET_UP: ScaProcSetUp = grass_tick_set_up;
