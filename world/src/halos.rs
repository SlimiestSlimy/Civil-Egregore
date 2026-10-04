//! Halos: the superchunks kept hot. Every entity of a kind that keeps a
//! halo ([`HALO_KEEPERS`]) keeps its own superchunk and the eight about
//! it hot -- its **halo** -- so whatever it reaches in a tick, as far as
//! the speed of light, is hot. Every other superchunk is cold: its
//! cells in its image in chunk storage, its entities and random
//! numbers kept as a save keeps them ([`crate::World::cold`]).
//!
//! The halos move after every tick ([`crate::World::tick`]), and
//! nothing slow is done on the tick: the background
//! ([`crate::background`]) does it.
//!
//! - A superchunk no halo reaches goes cold at once: its state kept, its
//!   bitmaps set aside, cooling, and its changed ones encoded in the
//!   background, then put into the writeback ring, which flushes them
//!   into its image when it needs the room.
//! - A superchunk a halo reaches, not hot, is **warming** for
//!   [`WARM_TICKS`] ticks: cooling, it is kept to be made hot as it is;
//!   else its image -- generated, if it was never made -- is decoded in
//!   the background. It turns hot at the tick it is due, waiting for the
//!   background if need be, so the world is the same however fast the
//!   background is. Until then it is not hot: writes to it are missed,
//!   and entities sent to it stay where they stood.
//!
//! So between ticks the hot superchunks are the halos less those
//! warming.

use crate::background::{Done, Job, Ticket};
use crate::World;
use coordinates::SuperchunkIndex;
use entity_rules::sheep::SHEEP;
use simulation::entity_store::{saved, EntityType};
use std::ops::AddAssign;

/// The kinds of entity that keep a halo: the ones that matter, about
/// which the world is ticked. For now the sheep -- the only kind there
/// is -- stand in for the people to come.
pub const HALO_KEEPERS: [EntityType; 1] = [SHEEP];

/// Ticks a superchunk a halo reaches is warming before it turns hot:
/// the background's time to make it. A keeper reaches a superchunk its
/// halo has just reached no sooner than it walks across its own, 1,024
/// cells -- a sheep steps once in `STEP_TICKS` (64) ticks or more -- so
/// long after.
pub const WARM_TICKS: u64 = 1024;

/// What moving the halos did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloChange {
    /// Superchunks a halo reached: warming.
    pub reached: usize,
    /// Superchunks made hot for the first time: generated.
    pub generated: usize,
    /// Superchunks made hot again, as they were: cooling, or from storage.
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
    /// Its bitmaps, cooling: made hot as they are.
    Cooling,
    /// The background, decoding its image: the job's ticket.
    Background(Ticket),
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
    /// they reach warming, hot [`WARM_TICKS`] on; the rest cold.
    pub fn move_halos(&mut self) -> HaloChange {
        let wanted = about(self.keepers());
        self.make_hot_within(&wanted, WARM_TICKS)
    }

    /// Makes `wanted` -- sorted -- the hot superchunks now: every other
    /// one made cold, its state kept; every one of them not hot made
    /// hot, cooling, from storage and its kept state, or generated --
    /// those from the background made at once, on all its threads.
    /// What generating and loading start from; between two ticks,
    /// anything else needing superchunks hot a while may ask too.
    pub fn keep_hot(&mut self, wanted: &[SuperchunkIndex]) -> HaloChange {
        self.make_hot_within(wanted, 0)
    }

    /// The superchunks warming, each with the tick it turns hot at.
    pub fn warming(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.warming.iter().map(|warming| (warming.superchunk, warming.due))
    }

    /// Makes `wanted` -- sorted -- the superchunks hot or warming, each
    /// warming hot within `ticks`: every other one made cold, or no
    /// longer warming; then every one due made hot.
    fn make_hot_within(&mut self, wanted: &[SuperchunkIndex], ticks: u64) -> HaloChange {
        self.land_write_backs(false);
        let (now, mut change) = (self.entities.now(), HaloChange::default());
        let hot = self.arena.superchunk_indices();
        let random: Vec<(SuperchunkIndex, u64)> = self.simulation.random_states().collect();
        for &superchunk in hot.iter().filter(|superchunk| wanted.binary_search(superchunk).is_err()) {
            let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
            self.cold.insert(superchunk, saved::encode_state(state, self.entities.superchunk(superchunk)).0);
            let dirty = self.arena.make_cold_superchunk(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.background.send(Job::Encode(dirty))));
            }
            change.cooled += 1;
        }
        for unwanted in self.warming.extract_if(.., |warming| wanted.binary_search(&warming.superchunk).is_err()) {
            match unwanted.from {
                WarmedFrom::Cooling => self.arena.let_go(unwanted.superchunk),
                WarmedFrom::Background(ticket) => self.background.forget(ticket),
            }
        }
        self.warming.iter_mut().for_each(|warming| warming.due = warming.due.min(now + ticks));
        for &superchunk in wanted.iter().filter(|superchunk| hot.binary_search(superchunk).is_err()) {
            if self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_err() {
                self.start_warming(superchunk, now + ticks);
                change.reached += 1;
            }
        }
        let due: Vec<Warming> = self.warming.extract_if(.., |warming| warming.due <= now).collect();
        if change.cooled == 0 && due.is_empty() {
            return change;
        }
        for warming in &due {
            self.finish_warming(warming, &mut change);
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
    /// it is if it is cooling, else sent to the background.
    pub(crate) fn start_warming(&mut self, superchunk: SuperchunkIndex, due: u64) {
        let from = if self.arena.hold(superchunk) {
            WarmedFrom::Cooling
        } else {
            let job = Job::Warm { superchunk, image: self.storage.shared_image(superchunk), seed: self.info.seed, types: self.info.layers.clone() };
            WarmedFrom::Background(self.background.send(job))
        };
        let at = self.warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).expect_err("not warming");
        self.warming.insert(at, Warming { superchunk, due, from });
    }

    /// Makes `warming`'s bitmaps hot -- as they were, or as the
    /// background made them, waiting for it if need be -- counted in
    /// `change`.
    fn finish_warming(&mut self, warming: &Warming, change: &mut HaloChange) {
        match warming.from {
            WarmedFrom::Cooling => {
                self.arena.make_hot_again(warming.superchunk);
                change.restored += 1;
            }
            WarmedFrom::Background(ticket) => {
                let Done::Warmed { generated, cells } = self.background.take(ticket) else {
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

    /// Puts the write-backs the background has encoded into the
    /// writeback ring, in the order they were taken -- each one waited
    /// for, if `wait`, else up to the first not yet encoded.
    pub(crate) fn land_write_backs(&mut self, wait: bool) {
        while let Some(&(superchunk, ticket)) = self.writing_back.front() {
            let done = if wait { Some(self.background.take(ticket)) } else { self.background.try_take(ticket) };
            let Some(Done::Encoded(encoded)) = done else {
                break;
            };
            self.arena.written_back(superchunk, &encoded, &mut self.storage);
            self.writing_back.pop_front();
        }
    }

    /// Writes back every hot superchunk's changed bitmaps, encoded on
    /// all the background's threads, and puts them, and every write-back
    /// still on its way, into the writeback ring.
    pub(crate) fn write_back_all(&mut self) {
        for superchunk in self.arena.superchunk_indices() {
            let dirty = self.arena.take_dirty(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.background.send(Job::Encode(dirty))));
            }
        }
        self.land_write_backs(true);
    }
}
