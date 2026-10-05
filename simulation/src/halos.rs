//! Halos: the superchunks kept hot. Every hot entity
//! ([`crate::hot::Hot`]) keeps its own superchunk and the eight about
//! it hot -- its **halo** -- so whatever it reaches in a tick, as far as
//! the speed of light, is hot. Every other superchunk is cold: its
//! cells in its image in chunk storage, its entities and random
//! numbers kept as a save keeps them ([`Held::cold`]).
//!
//! The halos move after every tick ([`Halos::move_to_hot_entities`]), and
//! nothing slow is done on the tick: chunk storage's jobs
//! ([`chunk_storage::jobs`]) do it, on the threads the tick has no use
//! for just then.
//!
//! - A hot superchunk no halo reaches is **cooling** for [`COOL_TICKS`]
//!   ticks: hot still, so a hot entity stepping back and forth over a
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

//!
//! What they work on is whoever holds the world's ([`Held`]): the
//! arena, chunk storage, the entities, the simulation and the cold
//! states, lent for each call, with what generates a superchunk never
//! made -- the one thing of the world's making halos need.

use crate::hot::Hot;
use crate::Simulation;
use bitplane_manager::BitmapArena;
use chunk_storage::jobs::{Done, Generate, Job, Jobs, Ticket};
use chunk_storage::{ChunkStorage, LayerType};
use coordinates::SuperchunkIndex;
use entity_manager::{saved, Entities};
use std::collections::{BTreeMap, VecDeque};
use std::ops::AddAssign;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;

/// Ticks a superchunk a halo reaches is warming before it turns hot:
/// a job's time to make it. Kept short, so the halos follow
/// their hot entities closely: a superchunk generated takes a job
/// longer, and the tick waits for it. A hot entity reaches a superchunk its
/// halo has just reached no sooner than it walks across its own, 1,024
/// cells -- a sheep steps once in `STEP_TICKS` (64) ticks or more -- so
/// long after.
pub const WARM_TICKS: u64 = 256;

/// Ticks a hot superchunk no halo reaches is cooling before it goes
/// cold: as long as one warming takes, so a hot entity stepping back over
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

/// The world's, lent to the halos for a call.
pub struct Held<'a> {
    /// Its hot bitmaps.
    pub arena: &'a mut BitmapArena,
    /// Every superchunk ever made, as stored.
    pub storage: &'a mut ChunkStorage,
    /// The hot superchunks' entities.
    pub entities: &'a mut Entities,
    /// Its simulation: the hot superchunks' random numbers.
    pub simulation: &'a mut Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it ([`saved::encode_state`]).
    pub cold: &'a mut BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its layer types: made hot on every chunk.
    pub layers: &'a [LayerType],
    /// What generates a superchunk never made.
    pub generate: &'a dyn Fn(SuperchunkIndex) -> Generate,
}

/// The halos: which superchunks are to be hot, those warming and
/// cooling with the ticks they are due, and the jobs making them.
pub struct Halos {
    /// Which superchunks are to be hot: the world's size, and the hot
    /// entity.
    pub hot: Hot,
    /// The threads encoding, generating and decoding off the tick.
    jobs: Jobs,
    /// The superchunks warming, sorted.
    warming: Vec<Warming>,
    /// The hot superchunks cooling, sorted, each with the tick it goes
    /// cold at.
    cooling: Vec<(SuperchunkIndex, u64)>,
    /// The write-backs of superchunks gone cold, each with its job,
    /// being encoded by chunk storage's jobs, in the order taken.
    writing_back: VecDeque<(SuperchunkIndex, Ticket)>,
    /// The superchunks whose changes were taken from the ring, each with
    /// its job, their images being rewritten by a job.
    flushing: Vec<(SuperchunkIndex, Ticket)>,
}

/// A superchunk warming: a halo reached it, and it turns hot at `due`.
#[derive(Clone, Copy, Debug)]
struct Warming {
    /// The superchunk.
    superchunk: SuperchunkIndex,
    /// The tick it turns hot at.
    due: u64,
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

impl Halos {
    /// No halos yet, as `hot` says which superchunks are to be, their
    /// jobs on `dispatcher`'s threads.
    pub fn new(hot: Hot, dispatcher: Arc<Dispatcher>) -> Self {
        Self { hot, jobs: Jobs::new(dispatcher), warming: Vec::new(), cooling: Vec::new(), writing_back: VecDeque::new(), flushing: Vec::new() }
    }

    /// Takes up `cooling` -- sorted, each with the tick it goes cold
    /// at -- as the hot superchunks cooling: as a save kept them.
    pub fn restore_cooling(&mut self, cooling: Vec<(SuperchunkIndex, u64)>) {
        self.cooling = cooling;
    }

    /// Moves the halos to where the hot entities stand
    /// ([`Hot::wanted`]): the superchunks
    /// reached warming, hot [`WARM_TICKS`] on; the rest cooling, cold
    /// [`COOL_TICKS`] on. Where it says nothing -- superchunks forced
    /// hot, in a world of no size -- nothing moves.
    pub fn move_to_hot_entities(&mut self, held: &mut Held<'_>) -> HaloChange {
        match self.hot.wanted(held.entities) {
            Some(wanted) => self.make_hot_within(held, &wanted, WARM_TICKS, COOL_TICKS),
            None => HaloChange::default(),
        }
    }

    /// Makes `wanted` -- sorted -- the hot superchunks now: every other
    /// one made cold, its state kept; every one of them not hot made
    /// hot, lingering, from storage and its kept state, or generated --
    /// those of jobs made at once, on every thread.
    /// What generating and loading start from; between two ticks,
    /// anything else needing superchunks hot a while may ask too.
    pub fn keep_hot(&mut self, held: &mut Held<'_>, wanted: &[SuperchunkIndex]) -> HaloChange {
        let mut change = self.make_hot_within(held, wanted, 0, 0);
        // One that was warming and is not wanted turned hot, as every warming does: cold now.
        if !self.cooling.is_empty() {
            change += self.make_hot_within(held, wanted, 0, 0);
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
    fn make_hot_within(&mut self, held: &mut Held<'_>, wanted: &[SuperchunkIndex], warm_ticks: u64, cool_ticks: u64) -> HaloChange {
        // Nothing outside the world's size, whoever wants it.
        let within: Vec<SuperchunkIndex>;
        let wanted = match self.hot.side {
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

    /// Puts the write-backs the jobs have encoded into the
    /// writeback ring, in the order they were taken -- each one waited
    /// for, if `wait`, else up to the first not yet encoded -- the
    /// superchunk at the ring's tail sent to be flushed whenever it needs
    /// the room; then puts the images flushed so far in the cold pool.
    pub fn land_write_backs(&mut self, held: &mut Held<'_>, wait: bool) {
        while let Some(&(superchunk, ticket)) = self.writing_back.front() {
            let done = if wait { Some(self.jobs.take(ticket)) } else { self.jobs.try_take(ticket) };
            let Some(Done::Encoded(encoded)) = done else {
                break;
            };
            for (key, words) in &encoded {
                while !held.storage.try_write_back(key.chunk, key.layer_type, words) {
                    self.flush_tail(held);
                }
            }
            held.arena.written_back(superchunk, encoded.iter().map(|(key, _)| *key));
            self.writing_back.pop_front();
        }
        self.land_flushes(held, wait);
    }

    /// Takes the changes of the superchunk at the ring's tail out of it,
    /// to rewrite its image by a job -- its flush before, if
    /// still on its way, put in the cold pool first, so each rewrites the
    /// image the one before made. Its buckets, hot or lingering, hold its
    /// newest cells until the image is in ([`Halos::land_flushes`]).
    fn flush_tail(&mut self, held: &mut Held<'_>) {
        let superchunk = held.storage.tail_superchunk().expect("a full ring has a tail");
        if let Some(at) = self.flushing.iter().position(|&(flushing, _)| flushing == superchunk) {
            let (_, ticket) = self.flushing.remove(at);
            let Done::Flushed(image) = self.jobs.take(ticket) else {
                unreachable!("a flush's job makes an image");
            };
            held.storage.insert(superchunk, image);
        }
        let flush = held.storage.take(superchunk).expect("the tail's changes");
        self.flushing.push((superchunk, self.jobs.send(Job::Flush(flush))));
    }

    /// Puts the images the jobs have rewritten in the cold pool --
    /// each waited for, if `wait` -- and tells the arena of each
    /// superchunk with none of its changes left in the ring, so its
    /// buckets waiting there may go.
    fn land_flushes(&mut self, held: &mut Held<'_>, wait: bool) {
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
            held.storage.insert(superchunk, image);
            if !held.storage.holds_changes(superchunk) {
                flushed.push(superchunk);
            }
        }
        held.arena.flushed(&flushed);
    }

    /// Flushes every superchunk with changes in the ring, rewritten on
    /// every thread: the ring empty, and the cold pool's
    /// images the cells written back.
    pub fn flush_all(&mut self, held: &mut Held<'_>) {
        while held.storage.tail_superchunk().is_some() {
            self.flush_tail(held);
        }
        self.land_flushes(held, true);
    }

    /// Writes back every hot superchunk's changed bitmaps, encoded on
    /// every thread, and puts them, and every write-back
    /// still on its way, into the writeback ring.
    pub fn write_back_all(&mut self, held: &mut Held<'_>) {
        for superchunk in held.arena.superchunk_indices() {
            let dirty = held.arena.take_dirty(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send(Job::Encode(dirty))));
            }
        }
        self.land_write_backs(held, true);
    }
}
