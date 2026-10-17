//! The world's halos: the simulation's (`simulation::halos`), lent
//! what the world holds and what generates a superchunk never made --
//! and, if its camera loads superchunks, a flock put on each one
//! generated in the viewport. Which are hot, the viewport's among them,
//! is the simulation's to say, as the world's hot defines it.

use crate::World;
use worldgen::generate_superchunk;
use chunk_storage::jobs::GenerateImage;
use chunk_storage::LayerCodec;
use coordinates::SuperchunkIndex;
use entity_manager::EntityType;
use entity_rules::sheep::SHEEP;
use simulation::halos::{HaloChange, Halos, LentWorld};

/// The hot entity, as a world's halos are told: the kind that
/// matters, about which the world is ticked. For now the sheep -- the
/// only kind there is -- stand in for the people to come.
pub const HOT_ENTITY: EntityType = SHEEP;

impl World {
    /// Has the halos do `halo_work` on what the world holds.
    fn with_halos<WorkResult>(&mut self, halo_work: impl FnOnce(&mut Halos, &mut LentWorld<'_>) -> WorkResult) -> WorkResult {
        let (seed, world_generation) = (self.world_info.seed, self.world_generation);
        let generate_job_of = move |superchunk: SuperchunkIndex| -> GenerateImage { Box::new(move |layer_codec: &mut LayerCodec| generate_superchunk(&world_generation, seed, superchunk).image(layer_codec)) };
        let mut world_held = LentWorld { bitmap_arena: &mut self.bitmap_arena, chunk_storage: &mut self.chunk_storage, entity_store: &mut self.entity_store, world_simulation: &mut self.world_simulation, cold: &mut self.cold, layers: &self.world_info.layers, generate_superchunk: &generate_job_of };
        halo_work(&mut self.halos, &mut world_held)
    }

    /// Moves the halos to where the hot entities stand, with the
    /// viewport's superchunks -- and, if its camera loads superchunks,
    /// puts its camera flock on each generated in the viewport, those
    /// out of view owed theirs (`docs/server.md`, "Halos").
    pub fn move_halos(&mut self) -> HaloChange {
        let halo_change = self.with_halos(|halos, world_held| halos.move_to_hot_entities(world_held));
        if let Some(camera_flock_sheep) = self.world_info.camera_flock.filter(|&sheep_in_flock| sheep_in_flock > 0) {
            let generated_superchunks = self.halos.generated_superchunks().to_vec();
            self.owe_camera_flocks(&generated_superchunks);
            // Paid to the hot ones in view: a superchunk still warming is paid once it is hot.
            let viewport = self.halos.viewport();
            let paid_superchunks: Vec<SuperchunkIndex> = self.world_info.without_camera_flock.iter().map(|&owed_index| SuperchunkIndex(owed_index)).filter(|superchunk| viewport.binary_search(superchunk).is_ok() && self.entity_store.superchunk(*superchunk).is_some()).collect();
            for &superchunk in &paid_superchunks {
                self.add_flock(superchunk, camera_flock_sheep as usize);
            }
            if !paid_superchunks.is_empty() {
                self.world_info.without_camera_flock.retain(|&still_owed_index| paid_superchunks.binary_search(&SuperchunkIndex(still_owed_index)).is_err());
                self.entity_store.apply();
            }
        }
        halo_change
    }

    /// Keeps `superchunks` as owed their camera flock
    /// (`WorldInfo::without_camera_flock`): generated, and not yet
    /// given one.
    pub(crate) fn owe_camera_flocks(&mut self, superchunks: &[SuperchunkIndex]) {
        let owed_superchunks = &mut self.world_info.without_camera_flock;
        owed_superchunks.extend(superchunks.iter().map(|superchunk| superchunk.0));
        owed_superchunks.sort_unstable();
        owed_superchunks.dedup();
    }

    /// Makes `wanted_hot` -- sorted -- the hot superchunks now, none
    /// outside the world's size (`Halos::keep_hot`): what generating
    /// and loading start from.
    pub fn keep_hot(&mut self, wanted_hot: &[SuperchunkIndex]) -> HaloChange {
        self.with_halos(|halos, world_held| halos.keep_hot(world_held, wanted_hot))
    }

    /// Pages cold superchunks' images out of memory while the world
    /// holds more of them than it keeps (`docs/server.md`, "The cold
    /// pool paged"; `Halos::page_cold_pool_out`). If the disk refuses
    /// one, it says so once and pages out no more.
    pub fn page_cold_pool_out(&mut self) {
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

    /// Starts warming `superchunk`, to turn hot at tick `due_tick`.
    pub(crate) fn start_warming(&mut self, superchunk: SuperchunkIndex, due_tick: u64) {
        self.with_halos(|halos, world_held| halos.start_warming(world_held, superchunk, due_tick));
    }

    /// Puts every write-back still on its way into the ring and
    /// flushes it: each cold superchunk's image the cells it went cold
    /// with. No hot superchunk's changed bitmaps are encoded: they stay
    /// as they are, hot, until it cools or the world is saved
    /// ([`World::write_back_and_flush_all`]).
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
