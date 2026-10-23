//! The world's halos at work: lent what the world holds and what
//! generates a superchunk never made -- its generation's code -- each
//! superchunk generated left unpopulated until its first tick hot.
//! Which are hot, the viewport's among them, is the halos' to say, as
//! the world's hot defines it.

use super::SimulationHandle;
use crate::simulation::halos::{HaloChange, Halos, LentWorld};
use protocol::pipe::frame::Viewport;
use crate::chunk_storage::jobs::GenerateImage;
use crate::chunk_storage::{GeneratedArea, LayerCodec};
use utilities::coordinates::SuperchunkIndex;
use std::sync::Arc;

impl SimulationHandle {
    /// Has the halos do `halo_work` on what the world holds.
    fn with_halos<WorkResult>(&mut self, halo_work: impl FnOnce(&mut Halos, &mut LentWorld<'_>) -> WorkResult) -> WorkResult {
        let (seed, generate_cells, worldgen_parameters) = (self.seed, Arc::clone(&self.worldgen.generate_cells), Arc::clone(&self.worldgen_parameters));
        let generate_job_of = move |superchunk: SuperchunkIndex| -> GenerateImage {
            let (generate_cells, worldgen_parameters) = (Arc::clone(&generate_cells), Arc::clone(&worldgen_parameters));
            Box::new(move |layer_codec: &mut LayerCodec| {
                let mut generated_area = GeneratedArea::of_superchunk(superchunk);
                generate_cells(&worldgen_parameters, seed, &mut generated_area);
                generated_area.image(layer_codec)
            })
        };
        let mut world_held = LentWorld { bitmap_arena: &mut self.bitmap_arena, chunk_storage: &mut self.chunk_storage, entity_store: &mut self.entity_store, world_simulation: &mut self.world_simulation, cold: &mut self.cold, layers: &self.layers, generate_superchunk: &generate_job_of };
        halo_work(&mut self.halos, &mut world_held)
    }

    /// Moves the halos to where the hot entities stand, with the
    /// viewport's superchunks if its camera forces them hot -- each superchunk
    /// generated left to be populated its first tick hot.
    pub(super) fn move_halos(&mut self) -> HaloChange {
        let halo_change = self.with_halos(|halos, world_held| halos.move_to_hot_entities(world_held));
        let generated_superchunks = self.halos.generated_superchunks().to_vec();
        self.mark_unpopulated(&generated_superchunks);
        halo_change
    }

    /// Makes `wanted_hot` -- sorted -- the hot superchunks now, none
    /// outside the world's size (`Halos::keep_hot`): what starting
    /// and loading build on.
    pub fn keep_hot(&mut self, wanted_hot: &[SuperchunkIndex]) -> HaloChange {
        self.with_halos(|halos, world_held| halos.keep_hot(world_held, wanted_hot))
    }

    /// Keeps the superchunks `viewport` shows hot, if the world's
    /// camera forces them hot: asked for the next time the halos move.
    pub fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        self.halos.keep_viewport(viewport);
    }

    /// Pages cold superchunks' images out of memory while the world
    /// holds more of them than it keeps (`Halos::page_cold_pool_out`).
    /// If the disk refuses one, it says so once and pages out no more.
    pub(super) fn page_cold_pool_out(&mut self) {
        if !self.chunk_storage.over_memory_kept() {
            return;
        }
        if let Err(paging_error) = self.with_halos(|halos, world_held| halos.page_cold_pool_out(world_held)) {
            eprintln!("The cold pool is no longer paged to disk, and grows in memory: {paging_error}");
            self.chunk_storage.stop_paging_out();
        }
    }

    /// The superchunks warming, each with the tick it turns hot at.
    pub fn warming(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.halos.warming()
    }

    /// The hot superchunks cooling, each with the tick it goes cold at.
    pub fn cooling(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.halos.cooling()
    }

    /// Starts warming `superchunk`, to turn hot at tick `due_tick`: a
    /// load's, as it was saved.
    pub fn start_warming(&mut self, superchunk: SuperchunkIndex, due_tick: u64) {
        self.with_halos(|halos, world_held| halos.start_warming(world_held, superchunk, due_tick));
    }

    /// Has `cooling` -- each superchunk with the tick it goes cold at --
    /// cooling again: a load's, as it was saved.
    pub fn restore_cooling(&mut self, cooling: Vec<(SuperchunkIndex, u64)>) {
        self.halos.restore_cooling(cooling);
    }

    /// Puts every write-back still on its way into the ring and
    /// flushes it: each cold superchunk's image the cells it went cold
    /// with. No hot superchunk's changed bitmaps are encoded: they stay
    /// as they are, hot, until it cools or the world is saved
    /// ([`SimulationHandle::write_back_and_flush_all`]).
    pub fn flush_what_went_cold(&mut self) {
        self.with_halos(|halos, world_held| {
            halos.land_write_backs(world_held, true);
            halos.flush_all(world_held);
        });
    }

    /// Writes back every hot superchunk's changed bitmaps, then
    /// flushes every superchunk with changes in the ring: the cold
    /// pool's images the cells as they are.
    pub fn write_back_and_flush_all(&mut self) {
        self.with_halos(|halos, world_held| {
            halos.write_back_all(world_held);
            halos.flush_all(world_held);
        });
    }
}
