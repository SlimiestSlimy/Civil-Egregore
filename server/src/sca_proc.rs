//! A sca proc as the server calls it ([`ScaProc`]), and its set up's
//! signature ([`ScaProcSetUp`]): a mod exports a static of it,
//! unmangled, by the name its premanifest gives -- the signature held
//! by the mod's own build.

use crate::server_handle::ServerHandle;
use protocol::type_registry::TypeRegistry;
use utilities::coordinates::CellIndex;

/// A sca proc: called on every tile of the planes it is sampled on.
pub trait ScaProc: Send + Sync {
    /// The tile whose corner cell is `tile_corner`, of the plane of type
    /// ID `sampled_plane_type_id` -- one of those it is sampled on -- seen
    /// through `server`.
    fn call(&self, server: &mut ServerHandle, sampled_plane_type_id: u64, tile_corner: CellIndex);
}

/// A sca proc's set up: the proc, made with what it takes of the
/// registry -- what it names there checked by its manifest, a set up
/// that cannot make its proc a bug of its mod's, a panic. A mod exports
/// one as `#[unsafe(no_mangle)] pub static NAME: ScaProcSetUp =
/// its_function;`.
pub type ScaProcSetUp = fn(&TypeRegistry) -> Box<dyn ScaProc>;
