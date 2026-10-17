//! Superchunks made hot and let cool: those wanted set against those
//! hot, the new ones warming on the jobs' threads, the unwanted cooling.

use super::{HaloChange, Halos, LentWorld, WarmedFrom, Warming};
use chunk_storage::jobs::{JobDone, Job};
use coordinates::SuperchunkIndex;
use entity_manager::saved;

impl Halos {
    /// Makes `wanted_superchunks` -- sorted -- the superchunks hot or warming, each
    /// warming hot within `warm_ticks`, each other hot one cold within
    /// `cool_ticks`; then every one due made hot, or cold. A warming is
    /// never given up (`docs/reference.md`, "Halos::make_hot_within").
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
        for warming in &due_warmings {
            self.finish_warming(lent_world, warming, &mut halo_change);
            // One no halo reaches any more turned hot all the same, and is cooling from now.
            if wanted_within_world.binary_search(&warming.superchunk).is_err() {
                let still_cooling_insert_index = self.cooling.binary_search_by_key(&warming.superchunk, |&(cooling, _)| cooling).expect_err("not hot till now");
                self.cooling.insert(still_cooling_insert_index, (warming.superchunk, current_tick + cool_ticks));
            }
        }
        let now_hot = lent_world.bitmap_arena.superchunk_indices();
        lent_world.entity_store.keep_superchunks(&now_hot);
        let mut put_back = Vec::new();
        for warming in &due_warmings {
            if let Some(words) = lent_world.cold.remove(&warming.superchunk) {
                let decoded_state = saved::decode_state(&words, current_tick, lent_world.entity_store).expect("a cold superchunk's state, as it was kept");
                put_back.extend(decoded_state.random_stream_state.map(|put_back_random_state| (warming.superchunk, put_back_random_state)));
            }
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

    /// Starts warming `superchunk`, to turn hot at tick `due_tick`: kept as
    /// it is if it is lingering, else sent as a job.
    pub(crate) fn start_warming(&mut self, lent_world: &mut LentWorld<'_>, superchunk: SuperchunkIndex, due_tick: u64) {
        let warmed_from = if lent_world.bitmap_arena.hold_lingering(superchunk) {
            WarmedFrom::Lingering
        } else {
            let job = Job::WarmSuperchunk { superchunk, image: lent_world.chunk_storage.stored_image(superchunk), generate_image: (lent_world.generate_superchunk)(superchunk), layer_types: lent_world.layers.to_vec() };
            WarmedFrom::Job(self.jobs.send_job(job))
        };
        let warming_insert_index = self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).expect_err("not warming");
        self.warming.insert(warming_insert_index, Warming { superchunk, due_tick, warmed_from });
    }

    /// Makes `warming`'s bitmaps hot -- as they were, or as the
    /// job made them, waiting for it if need be -- counted in
    /// `halo_change`.
    fn finish_warming(&mut self, lent_world: &mut LentWorld<'_>, warming: &Warming, halo_change: &mut HaloChange) {
        match warming.warmed_from {
            WarmedFrom::Lingering => {
                lent_world.bitmap_arena.make_hot_again(warming.superchunk);
                halo_change.superchunks_restored += 1;
            }
            WarmedFrom::Job(job_ticket) => {
                let JobDone::SuperchunkWarmed { generated_image: generated, image_read_back: read, cells } = self.jobs.take_waiting(job_ticket) else {
                    unreachable!("a warming's job makes a superchunk's cells");
                };
                // Paged to disk: in memory again while it is hot.
                if let Some(image) = read {
                    lent_world.chunk_storage.bring_in(warming.superchunk, image);
                }
                match generated {
                    Some(image) => {
                        lent_world.chunk_storage.insert(warming.superchunk, image);
                        let generated_insert_index = self.generated_superchunks.binary_search(&warming.superchunk).expect_err("generated once");
                        self.generated_superchunks.insert(generated_insert_index, warming.superchunk);
                        halo_change.superchunks_generated += 1;
                    }
                    None => halo_change.superchunks_restored += 1,
                }
                for (bucket_key, cells) in &cells {
                    lent_world.bitmap_arena.make_hot_cells(*bucket_key, cells.as_deref());
                }
            }
        }
    }
}
