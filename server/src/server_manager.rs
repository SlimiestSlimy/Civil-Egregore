//! The server manager ([`ServerManager`]): all the server holds of its
//! one world -- the registry it sees, the kinds' stores, and the hot
//! chunks -- one a process; and its tick ([`ServerManager::tick`]).

use crate::hot_chunk::HotChunk;
use crate::sca_proc::ScaProc;
use protocol::type_registry::TypeRegistry;
use utilities::chance::Chance;
use utilities::coordinates::ChunkIndex;

mod tick;

/// A sca proc, loaded: its code, and what it is sampled by.
pub struct LoadedScaProc {
    /// Its type ID.
    pub type_id: u64,
    /// The type IDs of the planes it is sampled on.
    pub sampled_plane_type_ids: Vec<u64>,
    /// The chance a tile not 0 of each of them is sampled for it, a tick.
    pub chance_per_tick: Chance,
    /// What its set up made.
    pub code: Box<dyn ScaProc>,
}

/// The server's world: its registry, its stores, its hot chunks.
pub struct ServerManager {
    /// The registry, the server's side's alone.
    type_registry: TypeRegistry,
    /// Every sca proc, in type ID order.
    sca_procs: Vec<LoadedScaProc>,
    /// The hot chunks, by chunk index: Morton order.
    hot_chunks: Vec<HotChunk>,
}

impl ServerManager {
    /// The world of the registry `type_registry` and the sca procs
    /// `sca_procs`, in type ID order: no chunk hot yet.
    pub fn new(type_registry: TypeRegistry, sca_procs: Vec<LoadedScaProc>) -> Self {
        Self { type_registry, sca_procs, hot_chunks: Vec::new() }
    }

    /// The registry, the server's side's alone.
    pub fn type_registry(&self) -> &TypeRegistry {
        &self.type_registry
    }

    /// Every sca proc, in type ID order.
    pub fn sca_procs(&self) -> &[LoadedScaProc] {
        &self.sca_procs
    }

    /// The chunk of index `chunk_index`, if it is hot.
    pub fn hot_chunk(&self, chunk_index: ChunkIndex) -> Option<&HotChunk> {
        self.hot_chunks.binary_search_by_key(&chunk_index.0, |chunk| chunk.chunk_index().0).ok().map(|place| &self.hot_chunks[place])
    }

    /// `chunk` made hot: a chunk hot already, a bug of the server's,
    /// panics.
    pub fn insert_hot_chunk(&mut self, chunk: HotChunk) {
        let place = self.hot_chunks.binary_search_by_key(&chunk.chunk_index().0, |hot| hot.chunk_index().0).expect_err("a chunk made hot twice");
        self.hot_chunks.insert(place, chunk);
    }
}
