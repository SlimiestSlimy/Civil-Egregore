//! Halos: the superchunks kept hot. Every entity of a kind that keeps a
//! halo ([`HALO_KEEPERS`]) keeps its own superchunk and the eight about
//! it hot -- its **halo** -- so whatever it reaches in a tick, as far as
//! the speed of light, is hot. Every other superchunk is cold: its
//! cells in its image in chunk storage, its entities and random
//! numbers kept as a save keeps them ([`crate::World::cold`]).
//!
//! The halos move after every tick ([`crate::World::tick`]), and
//! nothing slow is done on the tick: chunk storage's jobs
//! ([`chunk_storage::jobs`]) do it, on the threads the tick has no use
//! for just then.
//!
//! - A hot superchunk no halo reaches is **cooling** for [`COOL_TICKS`]
//!   ticks: hot still, so a keeper stepping back and forth over a
//!   superchunk's edge does not make the superchunks about it flicker
//!   cold and hot. Should a halo reach it again, it stays hot; else it
//!   goes cold at the tick it is due: its state kept, its bitmaps set
//!   aside, lingering, and its changed ones encoded by a job,
//!   then put into the writeback ring, which flushes them when it needs
//!   the room: the image rewritten by a job, the bitmaps held
//!   until it is in the cold pool.
//! - A superchunk a halo reaches, not hot, is **warming** for
//!   [`WARM_TICKS`] ticks, and nothing stops it: its halo gone again, it
//!   turns hot when due all the same, and is cooling from then.
//!   Lingering, it is kept to be made hot as it is;
//!   else its image -- generated, if it was never made -- is decoded in
//!   a job. It turns hot at the tick it is due, waiting for the job
//!   if need be, so the world is the same however fast the threads
//!   are. Until then it is, to the simulation, cold like any
//!   other: writes to it are missed, and entities sent to it stay where
//!   they stood. So is a superchunk lingering.
//!
//! So between ticks the hot superchunks are the halos less those
//! warming, and besides them those cooling; and one may be warming
//! that no halo reaches any more.

use crate::generate_image;
use chunk_storage::jobs::{Done, Job, Ticket};
use crate::World;
use coordinates::SuperchunkIndex;
use entity_rules::sheep::SHEEP;
use entity_manager::{saved, EntityType};
use std::ops::AddAssign;

/// The kinds of entity that keep a halo: the ones that matter, about
/// which the world is ticked. For now the sheep -- the only kind there
/// is -- stand in for the people to come.
pub const HALO_KEEPERS: [EntityType; 1] = [SHEEP];

/// Ticks a superchunk a halo reaches is warming before it turns hot:
/// a job's time to make it. Kept short, so the halos follow
/// their keepers closely: a superchunk generated takes a job
/// longer, and the tick waits for it. A keeper reaches a superchunk its
/// halo has just reached no sooner than it walks across its own, 1,024
/// cells -- a sheep steps once in `STEP_TICKS` (64) ticks or more -- so
/// long after.
pub const WARM_TICKS: u64 = 256;

/// Ticks a hot superchunk no halo reaches is cooling before it goes
/// cold: as long as one warming takes, so a keeper stepping back over
/// the edge it just crossed finds the superchunks behind it still hot.
pub const COOL_TICKS: u64 = 256;

/// What moving the halos did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloChange {
    /// Superchunks a halo reached: warming.
    pub reached: usize,
    /// Superchunks made hot for the first time: generated.
    pub generated: usize,
    /// Superchunks made hot again, as they were: lingering, or from storage.
    pub restored: usize,
    /// Superchunks gone cold.
    pub cooled: usize,
}

impl AddAssign for HaloChange {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.reached += other.reached;
        self.generated += other.generated;
        self.restored += other.restored;
        self.cooled += other.cooled;
    }
}

/// A superchunk warming: a halo reached it, and it turns hot at `due`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Warming {
    /// The superchunk.
    pub(crate) superchunk: SuperchunkIndex,
    /// The tick it turns hot at.
    pub(crate) due: u64,
    /// Where its cells come from.
    from: WarmedFrom,
}

/// Where a warming superchunk's cells come from.
#[derive(Clone, Copy, Debug)]
enum WarmedFrom {
    /// Its bitmaps, lingering: made hot as they are.
    Lingering,
    /// A job, decoding its image: its ticket.
    Job(Ticket),
}

/// The halos about the superchunks `keepers` -- each one's own and its
/// eight neighbours' -- as superchunk indices, sorted, each once.
pub fn about(keepers: impl Iterator<Item = SuperchunkIndex>) -> Vec<SuperchunkIndex> {
    let mut halos: Vec<SuperchunkIndex> = keepers.flat_map(|keeper| (-1..=1).flat_map(move |dy| (-1..=1).filter_map(move |dx| keeper.offset(dx, dy)))).collect();
    halos.sort_unstable();
    halos.dedup();
    halos
}

impl World {
    /// The hot superchunks holding an entity that keeps a halo.
    fn keepers(&self) -> impl Iterator<Item = SuperchunkIndex> + '_ {
        self.entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| HALO_KEEPERS.contains(&entity.header.kind))).map(|superchunk| superchunk.index())
    }

    /// Moves the halos to where their keepers stand: the superchunks
    /// they reach warming, hot [`WARM_TICKS`] on; the rest cooling, cold
    /// [`COOL_TICKS`] on.
    pub fn move_halos(&mut self) -> HaloChange {
        let wanted = about(self.keepers());
        self.make_hot_within(&wanted, WARM_TICKS, COOL_TICKS)
    }

    /// Makes `wanted` -- sorted -- the hot superchunks now: every other
    /// one made cold, its state kept; every one of them not hot made
    /// hot, lingering, from storage and its kept state, or generated --
    /// those of jobs made at once, on every thread.
    /// What generating and loading start from; between two ticks,
    /// anything else needing superchunks hot a while may ask too.
    pub fn keep_hot(&mut self, wanted: &[SuperchunkIndex]) -> HaloChange {
        let mut change = self.make_hot_within(wanted, 0, 0);
        // One that was warming and is not wanted turned hot, as every warming does: cold now.
        if !self.cooling.is_empty() {
            change += self.make_hot_within(wanted, 0, 0);
        }
        change
    }

    /// The superchunks warming, each with the tick it turns hot at.
    pub fn warming(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.warming.iter().map(|warming| (warming.superchunk, warming.due))
    }

    /// The hot superchunks cooling, each with the tick it goes cold at.
    pub fn cooling(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.cooling.iter().copied()
    }

    /// Makes `wanted` -- sorted -- the superchunks hot or warming, each
    /// warming hot within `warm_ticks`, each other one hot cold within
    /// `cool_ticks`, and those wanted again no longer cooling; then
    /// every one due made hot, or cold. A warming is never given up:
    /// one no longer wanted turns hot when it is due all the same, and
    /// is cooling from then -- so a superchunk's warming is one thing,
    /// whenever a save falls in it.
    fn make_hot_within(&mut self, wanted: &[SuperchunkIndex], warm_ticks: u64, cool_ticks: u64) -> HaloChange {
        self.land_write_backs(false);
        let (now, mut change) = (self.entities.now(), HaloChange::default());
        let hot = self.arena.superchunk_indices();
        self.cooling.retain(|(superchunk, _)| wanted.binary_search(superchunk).is_err());
        self.cooling.iter_mut().for_each(|(_, due)| *due = (*due).min(now + cool_ticks));
        for &superchunk in hot.iter().filter(|superchunk| wanted.binary_search(superchunk).is_err()) {
            if let Err(at) = self.cooling.binary_search_by_key(&superchunk, |&(cooling, _)| cooling) {
                self.cooling.insert(at, (superchunk, now + cool_ticks));
            }
        }
        let random: Vec<(SuperchunkIndex, u64)> = self.simulation.random_states().collect();
        for (superchunk, _) in self.cooling.extract_if(.., |&mut (_, due)| due <= now) {
            let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
            self.cold.insert(superchunk, saved::encode_state(state, self.entities.superchunk(superchunk)).0);
            let dirty = self.arena.make_cold_superchunk(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send(Job::Encode(dirty))));
            }
            change.cooled += 1;
        }
        self.warming.iter_mut().for_each(|warming| warming.due = warming.due.min(now + warm_ticks));
        for &superchunk in wanted.iter().filter(|superchunk| hot.binary_search(superchunk).is_err()) {
            if self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_err() {
                self.start_warming(superchunk, now + warm_ticks);
                change.reached += 1;
            }
        }
        let due: Vec<Warming> = self.warming.extract_if(.., |warming| warming.due <= now).collect();
        if change.cooled == 0 && due.is_empty() {
            return change;
        }
        for warming in &due {
            self.finish_warming(warming, &mut change);
            // One no halo reaches any more turned hot all the same, and is cooling from now.
            if wanted.binary_search(&warming.superchunk).is_err() {
                let at = self.cooling.binary_search_by_key(&warming.superchunk, |&(cooling, _)| cooling).expect_err("not hot till now");
                self.cooling.insert(at, (warming.superchunk, now + cool_ticks));
            }
        }
        let now_hot = self.arena.superchunk_indices();
        self.entities.align(&now_hot);
        let mut put_back = Vec::new();
        for warming in &due {
            if let Some(words) = self.cold.remove(&warming.superchunk) {
                let state = saved::decode_state(&words, now, &mut self.entities).expect("a cold superchunk's state, as it was kept");
                put_back.extend(state.random.map(|state| (warming.superchunk, state)));
            }
        }
        self.entities.apply();
        // The random streams of the superchunks hot all along, and of those made hot the ones kept: none of one gone cold.
        let made_hot = |superchunk: &SuperchunkIndex| due.iter().any(|warming| warming.superchunk == *superchunk);
        let kept = random.into_iter().filter(|state| now_hot.binary_search(&state.0).is_ok() && !made_hot(&state.0));
        let mut states: Vec<(SuperchunkIndex, u64)> = kept.chain(put_back).collect();
        states.sort_unstable();
        self.simulation.restore_random(&states);
        change
    }

    /// Starts warming `superchunk`, to turn hot at tick `due`: kept as
    /// it is if it is lingering, else sent as a job.
    pub(crate) fn start_warming(&mut self, superchunk: SuperchunkIndex, due: u64) {
        let from = if self.arena.hold(superchunk) {
            WarmedFrom::Lingering
        } else {
            let (seed, generation) = (self.info.seed, self.generation);
            let generate = Box::new(move |codec: &mut chunk_storage::LayerCodec| generate_image(&generation, seed, superchunk, codec));
            let job = Job::Warm { superchunk, image: self.storage.shared_image(superchunk), generate, types: self.info.layers.clone() };
            WarmedFrom::Job(self.jobs.send(job))
        };
        let at = self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).expect_err("not warming");
        self.warming.insert(at, Warming { superchunk, due, from });
    }

    /// Makes `warming`'s bitmaps hot -- as they were, or as the
    /// job made them, waiting for it if need be -- counted in
    /// `change`.
    fn finish_warming(&mut self, warming: &Warming, change: &mut HaloChange) {
        match warming.from {
            WarmedFrom::Lingering => {
                self.arena.make_hot_again(warming.superchunk);
                change.restored += 1;
            }
            WarmedFrom::Job(ticket) => {
                let Done::Warmed { generated, cells } = self.jobs.take(ticket) else {
                    unreachable!("a warming's job makes a superchunk's cells");
                };
                match generated {
                    Some(image) => {
                        self.storage.insert(warming.superchunk, image);
                        change.generated += 1;
                    }
                    None => change.restored += 1,
                }
                for (key, cells) in &cells {
                    self.arena.make_hot_cells(*key, cells.as_deref());
                }
            }
        }
    }

    /// Puts the write-backs the jobs have encoded into the
    /// writeback ring, in the order they were taken -- each one waited
    /// for, if `wait`, else up to the first not yet encoded -- the
    /// superchunk at the ring's tail sent to be flushed whenever it needs
    /// the room; then puts the images flushed so far in the cold pool.
    pub(crate) fn land_write_backs(&mut self, wait: bool) {
        while let Some(&(superchunk, ticket)) = self.writing_back.front() {
            let done = if wait { Some(self.jobs.take(ticket)) } else { self.jobs.try_take(ticket) };
            let Some(Done::Encoded(encoded)) = done else {
                break;
            };
            for (key, words) in &encoded {
                while !self.storage.try_write_back(key.chunk, key.layer_type, words) {
                    self.flush_tail();
                }
            }
            self.arena.written_back(superchunk, encoded.iter().map(|(key, _)| *key));
            self.writing_back.pop_front();
        }
        self.land_flushes(wait);
    }

    /// Takes the changes of the superchunk at the ring's tail out of it,
    /// to rewrite its image by a job -- its flush before, if
    /// still on its way, put in the cold pool first, so each rewrites the
    /// image the one before made. Its buckets, hot or lingering, hold its
    /// newest cells until the image is in ([`World::land_flushes`]).
    fn flush_tail(&mut self) {
        let superchunk = self.storage.tail_superchunk().expect("a full ring has a tail");
        if let Some(at) = self.flushing.iter().position(|&(flushing, _)| flushing == superchunk) {
            let (_, ticket) = self.flushing.remove(at);
            let Done::Flushed(image) = self.jobs.take(ticket) else {
                unreachable!("a flush's job makes an image");
            };
            self.storage.insert(superchunk, image);
        }
        let flush = self.storage.take(superchunk).expect("the tail's changes");
        self.flushing.push((superchunk, self.jobs.send(Job::Flush(flush))));
    }

    /// Puts the images the jobs have rewritten in the cold pool --
    /// each waited for, if `wait` -- and tells the arena of each
    /// superchunk with none of its changes left in the ring, so its
    /// buckets waiting there may go.
    fn land_flushes(&mut self, wait: bool) {
        let mut flushed = Vec::new();
        for (superchunk, ticket) in std::mem::take(&mut self.flushing) {
            let done = if wait { Some(self.jobs.take(ticket)) } else { self.jobs.try_take(ticket) };
            let Some(done) = done else {
                self.flushing.push((superchunk, ticket));
                continue;
            };
            let Done::Flushed(image) = done else {
                unreachable!("a flush's job makes an image");
            };
            self.storage.insert(superchunk, image);
            if !self.storage.holds_changes(superchunk) {
                flushed.push(superchunk);
            }
        }
        self.arena.flushed(&flushed);
    }

    /// Flushes every superchunk with changes in the ring, rewritten on
    /// every thread: the ring empty, and the cold pool's
    /// images the cells written back.
    pub(crate) fn flush_all(&mut self) {
        while self.storage.tail_superchunk().is_some() {
            self.flush_tail();
        }
        self.land_flushes(true);
    }

    /// Writes back every hot superchunk's changed bitmaps, encoded on
    /// every thread, and puts them, and every write-back
    /// still on its way, into the writeback ring.
    pub(crate) fn write_back_all(&mut self) {
        for superchunk in self.arena.superchunk_indices() {
            let dirty = self.arena.take_dirty(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send(Job::Encode(dirty))));
            }
        }
        self.land_write_backs(true);
    }
}
