//! Superchunks made hot and let cool: those wanted set against those
//! hot, the new ones warming on the jobs' threads, the unwanted cooling.

pub(crate) mod generating_superchunk;
pub(crate) mod warm_job;

use super::{HaloChange, Halos, LentWorld, Warming};
use crate::chunk_storage::jobs::Job;
use crate::chunk_storage::SuperchunkImage;
use utilities::coordinates::SuperchunkIndex;
use crate::entity_manager::saved;
use std::sync::Arc;
use warm_job::{warm_superchunk, WarmSource};

impl Halos {
    /// Makes `wanted_superchunks` -- sorted -- the superchunks hot or warming, each
    /// warming hot within `warm_ticks`, each other hot one cold within
    /// `cool_ticks`; then every one due made hot, or cold. A warming is
    /// never given up (`docs/simulation_reference.md`, "Halos::make_hot_within").
    pub(crate) fn make_hot_within(&mut self, lent_world: &mut LentWorld<'_>, wanted_superchunks: &[SuperchunkIndex], warm_ticks: u64, cool_ticks: u64) -> HaloChange {
        // Nothing outside the world's size, whoever wants it.
        let within_world_superchunks: Vec<SuperchunkIndex>;
        let wanted_within_world = match self.hot.world_side_superchunks() {
            Some(_) => {
                within_world_superchunks = wanted_superchunks.iter().copied().filter(|&superchunk| self.hot.is_within_world(superchunk)).collect();
                &within_world_superchunks[..]
            }
            None => wanted_superchunks,
        };
        self.land_write_backs(lent_world, false);
        let (current_tick, mut halo_change) = (lent_world.entity_store.current_tick(), HaloChange::default());
        let hot = lent_world.bitmap_arena.superchunk_indices();
        self.cooling.retain(|(superchunk, _)| wanted_within_world.binary_search(superchunk).is_err());
        self.cooling.iter_mut().for_each(|(_, cooling_due_tick)| *cooling_due_tick = (*cooling_due_tick).min(current_tick + cool_ticks));
        for &superchunk in hot.iter().filter(|superchunk| wanted_within_world.binary_search(superchunk).is_err()) {
            if let Err(cooling_insert_index) = self.cooling.binary_search_by_key(&superchunk, |&(cooling, _)| cooling) {
                self.cooling.insert(cooling_insert_index, (superchunk, current_tick + cool_ticks));
            }
        }
        let random_streams: Vec<(SuperchunkIndex, u64)> = lent_world.world_simulation.random_states().collect();
        for (superchunk, _) in self.cooling.extract_if(.., |&mut (_, cooled_due_tick)| cooled_due_tick <= current_tick) {
            let random_stream_state = random_streams.binary_search_by_key(&superchunk, |stream_entry| stream_entry.0).ok().map(|stream_index| random_streams[stream_index].1);
            lent_world.cold.insert(superchunk, saved::encode_state(random_stream_state, lent_world.entity_store.superchunk(superchunk)).0);
            let dirty = lent_world.bitmap_arena.make_cold_superchunk(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send_job(Job::EncodeWriteBack(dirty))));
            }
            halo_change.superchunks_cooled += 1;
        }
        self.warming.iter_mut().for_each(|warming| warming.due_tick = warming.due_tick.min(current_tick + warm_ticks));
        for &superchunk in wanted_within_world.iter().filter(|superchunk| hot.binary_search(superchunk).is_err()) {
            if self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_err() {
                self.start_warming(lent_world, superchunk, current_tick + warm_ticks);
                halo_change.superchunks_reached += 1;
            }
        }
        let due_warmings: Vec<Warming> = self.warming.extract_if(.., |warming| warming.due_tick <= current_tick).collect();
        if halo_change.superchunks_cooled == 0 && due_warmings.is_empty() {
            return halo_change;
        }
        let mut warmed_entities = Vec::with_capacity(due_warmings.len());
        let mut put_back = Vec::new();
        for warming in &due_warmings {
            // Waited for, if it is not done: the tick does not run on without it.
            let warmed = self.warm_jobs.take_waiting(warming.job);
            // Paged to disk: in memory again while it is hot.
            if let Some(image) = warmed.image_read_back {
                lent_world.chunk_storage.bring_in(warming.superchunk, image);
            }
            match warmed.cells {
                Some(cells) => lent_world.bitmap_arena.make_hot_warmed(cells),
                None => drop(lent_world.bitmap_arena.make_hot_again(warming.superchunk)),
            }
            if warmed.generated {
                // Made now, nothing written back yet: its buckets, all dirty, are its cells till it cools.
                lent_world.chunk_storage.insert(warming.superchunk, SuperchunkImage::empty());
                halo_change.superchunks_generated += 1;
            } else {
                halo_change.superchunks_restored += 1;
            }
            lent_world.cold.remove(&warming.superchunk);
            put_back.extend(warmed.random_stream_state.map(|random_state| (warming.superchunk, random_state)));
            warmed_entities.push(warmed.entities);
            // One no halo reaches any more turned hot all the same, and is cooling from now.
            if wanted_within_world.binary_search(&warming.superchunk).is_err() {
                let still_cooling_insert_index = self.cooling.binary_search_by_key(&warming.superchunk, |&(cooling, _)| cooling).expect_err("not hot till now");
                self.cooling.insert(still_cooling_insert_index, (warming.superchunk, current_tick + cool_ticks));
            }
        }
        let now_hot = lent_world.bitmap_arena.superchunk_indices();
        lent_world.entity_store.keep_superchunks(&now_hot);
        for superchunk_entities in warmed_entities {
            lent_world.entity_store.adopt_superchunk(superchunk_entities);
        }
        lent_world.entity_store.apply();
        // The random streams of the superchunks hot all along, and of those made hot the ones kept: none of one gone cold.
        let made_hot = |superchunk: &SuperchunkIndex| due_warmings.iter().any(|warming| warming.superchunk == *superchunk);
        let kept_random_streams = random_streams.into_iter().filter(|kept_stream_entry| now_hot.binary_search(&kept_stream_entry.0).is_ok() && !made_hot(&kept_stream_entry.0));
        let mut restored_random_streams: Vec<(SuperchunkIndex, u64)> = kept_random_streams.chain(put_back).collect();
        restored_random_streams.sort_unstable();
        lent_world.world_simulation.restore_random(&restored_random_streams);
        halo_change
    }

    /// Starts warming `superchunk`, to turn hot at tick `due_tick`: its
    /// job sent, building it whole from where it comes from -- cooled, its
    /// cells still in the arena; stored; or never made, generated.
    pub(crate) fn start_warming(&mut self, lent_world: &mut LentWorld<'_>, superchunk: SuperchunkIndex, due_tick: u64) {
        // Kept cold until it lands: a save while it warms reads it there.
        let entity_state = lent_world.cold.get(&superchunk).cloned();
        let source = if lent_world.bitmap_arena.hold_cooled(superchunk) {
            WarmSource::Cooled { entity_state }
        } else {
            match lent_world.chunk_storage.stored_image(superchunk) {
                Some(image) => WarmSource::Stored { image, entity_state },
                None => WarmSource::Generated((lent_world.generate_superchunk)(superchunk)),
            }
        };
        let (layer_types, default_attributes) = (lent_world.layers.to_vec(), Arc::clone(lent_world.entity_store.shared_default_attributes()));
        let job = self.warm_jobs.send_work(move |layer_codec| warm_superchunk(superchunk, source, &layer_types, due_tick, default_attributes, layer_codec));
        let warming_insert_index = self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).expect_err("not warming");
        self.warming.insert(warming_insert_index, Warming { superchunk, due_tick, job });
    }
}
