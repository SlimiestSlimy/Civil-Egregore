//! A tick of every hot chunk ([`ServerManager::tick`]), in two phases:
//! the procs called, every chunk read alone, each chunk's sets queued;
//! then each chunk's queue applied to it.

use super::ServerManager;
use crate::plane_sampling::sample_plane;
use crate::queue_applier::apply_queue;
use crate::queued_instruction::Instruction;
use crate::server_handle::ServerHandle;
use utilities::rng::RandomSource;

impl ServerManager {
    /// One tick: each hot chunk, in Morton order, its sca procs called --
    /// in type ID order, each on every plane it is sampled on -- on what
    /// the tick began with; then each chunk's queue applied, its random
    /// stream kept as drawn, its planes emptied let go.
    pub fn tick(&mut self) {
        let mut chunk_ticks: Vec<(RandomSource, Vec<Instruction>)> = Vec::with_capacity(self.hot_chunks.len());
        let mut hits = Vec::new();
        for chunk in &self.hot_chunks {
            let (mut random_stream, mut queue) = (chunk.take_random_stream(), Vec::new());
            let mut server_handle = ServerHandle::new(self, chunk.chunk_index(), &mut random_stream, &mut queue);
            for sca_proc in &self.sca_procs {
                for &plane_type_id in &sca_proc.sampled_plane_type_ids {
                    hits.clear();
                    sample_plane(chunk, plane_type_id, sca_proc.chance_per_tick, server_handle.random_stream(), &mut hits);
                    for &tile_corner in &hits {
                        sca_proc.code.call(&mut server_handle, plane_type_id, tile_corner);
                    }
                }
            }
            chunk_ticks.push((random_stream, queue));
        }
        for (chunk, (random_stream, queue)) in self.hot_chunks.iter_mut().zip(chunk_ticks) {
            chunk.give_back_random_stream(random_stream);
            apply_queue(chunk, &queue, &self.type_registry);
            chunk.drop_empty_planes();
        }
    }
}
