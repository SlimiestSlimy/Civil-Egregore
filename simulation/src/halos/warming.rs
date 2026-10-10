//! Superchunks made hot and let cool: those wanted set against those
//! hot, the new ones warming on the jobs' threads, the unwanted cooling.

use super::{HaloChange, Halos, Held, WarmedFrom, Warming};
use chunk_storage::jobs::{Done, Job};
use coordinates::SuperchunkIndex;
use entity_manager::saved;

impl Halos {
    /// Makes `wanted` -- sorted -- the superchunks hot or warming, each
    /// warming hot within `warm_ticks`, each other hot one cold within
    /// `cool_ticks`; then every one due made hot, or cold. A warming is
    /// never given up (`docs/reference.md`, "Halos::make_hot_within").
    pub(crate) fn make_hot_within(&mut self, held: &mut Held<'_>, wanted: &[SuperchunkIndex], warm_ticks: u64, cool_ticks: u64) -> HaloChange {
        // Nothing outside the world's size, whoever wants it.
        let within: Vec<SuperchunkIndex>;
        let wanted = match self.hot.side() {
            Some(_) => {
                within = wanted.iter().copied().filter(|&superchunk| self.hot.within(superchunk)).collect();
                &within[..]
            }
            None => wanted,
        };
        self.land_write_backs(held, false);
        let (now, mut change) = (held.entities.now(), HaloChange::default());
        let hot = held.arena.superchunk_indices();
        self.cooling.retain(|(superchunk, _)| wanted.binary_search(superchunk).is_err());
        self.cooling.iter_mut().for_each(|(_, due)| *due = (*due).min(now + cool_ticks));
        for &superchunk in hot.iter().filter(|superchunk| wanted.binary_search(superchunk).is_err()) {
            if let Err(at) = self.cooling.binary_search_by_key(&superchunk, |&(cooling, _)| cooling) {
                self.cooling.insert(at, (superchunk, now + cool_ticks));
            }
        }
        let random: Vec<(SuperchunkIndex, u64)> = held.simulation.random_states().collect();
        for (superchunk, _) in self.cooling.extract_if(.., |&mut (_, due)| due <= now) {
            let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
            held.cold.insert(superchunk, saved::encode_state(state, held.entities.superchunk(superchunk)).0);
            let dirty = held.arena.make_cold_superchunk(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send(Job::Encode(dirty))));
            }
            change.cooled += 1;
        }
        self.warming.iter_mut().for_each(|warming| warming.due = warming.due.min(now + warm_ticks));
        for &superchunk in wanted.iter().filter(|superchunk| hot.binary_search(superchunk).is_err()) {
            if self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_err() {
                self.start_warming(held, superchunk, now + warm_ticks);
                change.reached += 1;
            }
        }
        let due: Vec<Warming> = self.warming.extract_if(.., |warming| warming.due <= now).collect();
        if change.cooled == 0 && due.is_empty() {
            return change;
        }
        for warming in &due {
            self.finish_warming(held, warming, &mut change);
            // One no halo reaches any more turned hot all the same, and is cooling from now.
            if wanted.binary_search(&warming.superchunk).is_err() {
                let at = self.cooling.binary_search_by_key(&warming.superchunk, |&(cooling, _)| cooling).expect_err("not hot till now");
                self.cooling.insert(at, (warming.superchunk, now + cool_ticks));
            }
        }
        let now_hot = held.arena.superchunk_indices();
        held.entities.align(&now_hot);
        let mut put_back = Vec::new();
        for warming in &due {
            if let Some(words) = held.cold.remove(&warming.superchunk) {
                let state = saved::decode_state(&words, now, held.entities).expect("a cold superchunk's state, as it was kept");
                put_back.extend(state.random.map(|state| (warming.superchunk, state)));
            }
        }
        held.entities.apply();
        // The random streams of the superchunks hot all along, and of those made hot the ones kept: none of one gone cold.
        let made_hot = |superchunk: &SuperchunkIndex| due.iter().any(|warming| warming.superchunk == *superchunk);
        let kept = random.into_iter().filter(|state| now_hot.binary_search(&state.0).is_ok() && !made_hot(&state.0));
        let mut states: Vec<(SuperchunkIndex, u64)> = kept.chain(put_back).collect();
        states.sort_unstable();
        held.simulation.restore_random(&states);
        change
    }

    /// Starts warming `superchunk`, to turn hot at tick `due`: kept as
    /// it is if it is lingering, else sent as a job.
    pub fn start_warming(&mut self, held: &mut Held<'_>, superchunk: SuperchunkIndex, due: u64) {
        let from = if held.arena.hold(superchunk) {
            WarmedFrom::Lingering
        } else {
            let job = Job::Warm { superchunk, image: held.storage.shared_image(superchunk), generate: (held.generate)(superchunk), types: held.layers.to_vec() };
            WarmedFrom::Job(self.jobs.send(job))
        };
        let at = self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).expect_err("not warming");
        self.warming.insert(at, Warming { superchunk, due, from });
    }

    /// Makes `warming`'s bitmaps hot -- as they were, or as the
    /// job made them, waiting for it if need be -- counted in
    /// `change`.
    fn finish_warming(&mut self, held: &mut Held<'_>, warming: &Warming, change: &mut HaloChange) {
        match warming.from {
            WarmedFrom::Lingering => {
                held.arena.make_hot_again(warming.superchunk);
                change.restored += 1;
            }
            WarmedFrom::Job(ticket) => {
                let Done::Warmed { generated, cells } = self.jobs.take(ticket) else {
                    unreachable!("a warming's job makes a superchunk's cells");
                };
                match generated {
                    Some(image) => {
                        held.storage.insert(warming.superchunk, image);
                        let at = self.generated.binary_search(&warming.superchunk).expect_err("generated once");
                        self.generated.insert(at, warming.superchunk);
                        change.generated += 1;
                    }
                    None => change.restored += 1,
                }
                for (key, cells) in &cells {
                    held.arena.make_hot_cells(*key, cells.as_deref());
                }
            }
        }
    }
}
