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
//! If the world's hot says so ([`Hot::viewport`]), the superchunks of
//! the **viewport** -- what a renderer renders, told each frame
//! ([`Halos::keep_viewport`]) -- are hot too, every one of them,
//! warming and cooling as a halo's do: a world whose camera loads
//! superchunks, made where it is looked at. Whoever holds the world is
//! told which superchunks the halos' last move generated
//! ([`Halos::generated`]), to put on them what a superchunk never made
//! starts with.
//!
//! What they work on is whoever holds the world's ([`Held`]): the
//! arena, chunk storage, the entities, the simulation and the cold
//! states, lent for each call, with what generates a superchunk never
//! made -- the one thing of the world's making halos need.

mod warming;
mod write_back;

use crate::hot::Hot;
use crate::Simulation;
use bitplane_manager::BitmapArena;
use chunk_storage::jobs::{Generate, Jobs, Ticket};
use chunk_storage::{ChunkStorage, LayerType};
use coordinates::SuperchunkIndex;
use entity_manager::Entities;
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

/// What the renderer should render, in superchunks: a rectangle of
/// them, each `(x, y)` in superchunks from the world's top left, both
/// corners in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// The top left superchunk, `(x, y)`.
    pub first: (u32, u32),
    /// The bottom right one.
    pub last: (u32, u32),
}

impl Viewport {
    /// Whether `(x, y)`, in superchunks, is in it.
    pub fn contains(self, (x, y): (u32, u32)) -> bool {
        (self.first.0..=self.last.0).contains(&x) && (self.first.1..=self.last.1).contains(&y)
    }
}

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
    /// Which superchunks are to be hot: the world's size, the hot
    /// entity, and whether the viewport's are.
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
    /// The viewport last told, if the viewport's superchunks are hot.
    told: Option<Viewport>,
    /// Its superchunks within the world, sorted: wanted hot besides the
    /// halos.
    viewport: Vec<SuperchunkIndex>,
    /// The superchunks the last move generated, sorted.
    generated: Vec<SuperchunkIndex>,
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
        Self { hot, jobs: Jobs::new(dispatcher), warming: Vec::new(), cooling: Vec::new(), writing_back: VecDeque::new(), flushing: Vec::new(), told: None, viewport: Vec::new(), generated: Vec::new() }
    }

    /// Takes `viewport` -- what the renderer renders now, none if it
    /// renders none of the world's cells -- in place of the last: if
    /// the viewport's superchunks are hot ([`Hot::viewport`]), every one
    /// of it within the world is wanted hot from the next move on,
    /// besides the halos, warming and cooling as a halo's superchunks
    /// do. Else it is nothing to the halos.
    pub fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        let viewport = viewport.filter(|_| self.hot.viewport());
        if viewport == self.told {
            return;
        }
        self.told = viewport;
        self.viewport.clear();
        if let Some(viewport) = viewport {
            let span = self.hot.span();
            let (across, down) = (viewport.first.0.max(span.start)..=viewport.last.0.min(span.end - 1), viewport.first.1.max(span.start)..=viewport.last.1.min(span.end - 1));
            self.viewport.extend(down.flat_map(|y| across.clone().map(move |x| SuperchunkIndex::from_cartesian(x, y))));
            self.viewport.sort_unstable();
        }
    }

    /// The viewport's superchunks within the world, sorted: none unless
    /// they are hot ([`Halos::keep_viewport`]).
    pub fn viewport(&self) -> &[SuperchunkIndex] {
        &self.viewport
    }

    /// The superchunks the last move -- or [`Halos::keep_hot`] --
    /// generated, sorted: made hot for the first time, nothing on them.
    pub fn generated(&self) -> &[SuperchunkIndex] {
        &self.generated
    }

    /// Takes up `cooling` -- sorted, each with the tick it goes cold
    /// at -- as the hot superchunks cooling: as a save kept them.
    pub fn restore_cooling(&mut self, cooling: Vec<(SuperchunkIndex, u64)>) {
        self.cooling = cooling;
    }

    /// Moves the halos to where the hot entities stand
    /// ([`Hot::wanted`]), with the viewport's superchunks: the superchunks
    /// reached warming, hot [`WARM_TICKS`] on; the rest cooling, cold
    /// [`COOL_TICKS`] on. Forced hot, it is the whole world all the while.
    pub fn move_to_hot_entities(&mut self, held: &mut Held<'_>) -> HaloChange {
        let mut wanted = self.hot.wanted(held.entities);
        if !self.viewport.is_empty() {
            wanted.extend_from_slice(&self.viewport);
            wanted.sort_unstable();
            wanted.dedup();
        }
        self.generated.clear();
        self.make_hot_within(held, &wanted, WARM_TICKS, COOL_TICKS)
    }

    /// Makes `wanted` -- sorted -- the hot superchunks now: every other
    /// one made cold, its state kept; every one of them not hot made
    /// hot, lingering, from storage and its kept state, or generated --
    /// those of jobs made at once, on every thread.
    /// What generating and loading start from; between two ticks,
    /// anything else needing superchunks hot a while may ask too.
    pub fn keep_hot(&mut self, held: &mut Held<'_>, wanted: &[SuperchunkIndex]) -> HaloChange {
        self.generated.clear();
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
}
