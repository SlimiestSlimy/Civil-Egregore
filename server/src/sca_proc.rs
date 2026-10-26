//! A sca proc as the server calls it ([`ScaProc`]): what its set up
//! exports, boxed as a `Box<dyn ScaProc>`. The server knows it by this
//! trait alone; a mod depending on its mod knows it by its own type,
//! upcast to `&dyn Any` and downcast.

use crate::server_handle::ServerHandle;
use std::any::Any;
use utilities::coordinates::CellIndex;

/// A sca proc: called on every tile of the planes it is sampled on.
pub trait ScaProc: Any + Send + Sync {
    /// The tile whose corner cell is `tile_corner`, of the plane of type
    /// ID `sampled_plane_type_id` -- one of those it is sampled on -- seen
    /// through `server`.
    fn call(&self, server: &mut ServerHandle, sampled_plane_type_id: u64, tile_corner: CellIndex);
}
