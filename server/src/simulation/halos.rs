//! Halos: the superchunks kept hot about the hot entities, and the
//! viewport's if the world says so -- warming and cooling, each due at
//! a tick, the slow work done by chunk storage's jobs off the tick
//! (`docs/simulation.md`, "Halos").

mod warming;
mod write_back;

pub use warming::generating_superchunk::GeneratingSuperchunk;
pub(crate) use warming::warm_job::GenerateSuperchunk;
use warming::warm_job::WarmedSuperchunk;

use crate::simulation::hot::Hot;
use crate::simulation::Simulation;
use crate::bitplane_manager::BitmapArena;
use crate::chunk_storage::jobs::{JobDone, Jobs, JobTicket};
use crate::chunk_storage::{ChunkStorage, LayerType};
use utilities::coordinates::SuperchunkIndex;
use crate::simulation::simulation_time::SimulationTime;
use crate::entity_manager::WorldEntities;
use std::collections::{BTreeMap, VecDeque};
use std::ops::AddAssign;
use protocol::pipe::frame::Viewport;
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;

/// Ticks a superchunk a halo reaches is warming before it turns hot
/// (`docs/simulation.md`, "Halos").
pub const WARM_TICKS: u64 = SimulationTime::from_seconds(1).ticks();

/// Ticks a hot superchunk no halo reaches is cooling before it goes
/// cold: as long as one warming takes, so a hot entity stepping back over
/// the edge it just crossed finds the superchunks behind it still hot.
pub const COOL_TICKS: u64 = SimulationTime::from_seconds(1).ticks();

/// What moving the halos did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloChange {
    /// Superchunks a halo reached: warming.
    pub superchunks_reached: usize,
    /// Superchunks made hot for the first time: generated.
    pub superchunks_generated: usize,
    /// Superchunks made hot again, as they were: cooled, or from storage.
    pub superchunks_restored: usize,
    /// Superchunks gone cold.
    pub superchunks_cooled: usize,
}

impl AddAssign for HaloChange {
    /// Both added up.
    fn add_assign(&mut self, added_change: Self) {
        self.superchunks_reached += added_change.superchunks_reached;
        self.superchunks_generated += added_change.superchunks_generated;
        self.superchunks_restored += added_change.superchunks_restored;
        self.superchunks_cooled += added_change.superchunks_cooled;
    }
}

/// The world's, lent to the halos for a call.
pub(crate) struct LentWorld<'a> {
    /// Its hot bitmaps.
    pub(crate) bitmap_arena: &'a mut BitmapArena,
    /// Every superchunk ever made, as stored.
    pub(crate) chunk_storage: &'a mut ChunkStorage,
    /// The hot superchunks' entities.
    pub(crate) entity_store: &'a mut WorldEntities,
    /// Its simulation: the hot superchunks' random numbers.
    pub(crate) world_simulation: &'a mut Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it ([`saved::encode_state`]).
    pub(crate) cold: &'a mut BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its layer types: made hot on every chunk.
    pub(crate) layers: &'a [LayerType],
    /// What generates a superchunk never made.
    pub(crate) generate_superchunk: &'a dyn Fn(SuperchunkIndex) -> GenerateSuperchunk,
}

/// The halos: which superchunks are to be hot, those warming and
/// cooling with the ticks they are due, and the jobs making them.
pub struct Halos {
    /// Which superchunks are to be hot: the world's size, the hot
    /// entity, and whether the viewport's are.
    pub hot: Hot,
    /// Chunk storage's jobs: write-backs encoded, images rewritten.
    jobs: Jobs<JobDone>,
    /// The warmings' jobs: each superchunk warming built whole, off the tick.
    warm_jobs: Jobs<WarmedSuperchunk>,
    /// The superchunks warming, sorted.
    warming: Vec<Warming>,
    /// The hot superchunks cooling, sorted, each with the tick it goes
    /// cold at.
    cooling: Vec<(SuperchunkIndex, u64)>,
    /// The write-backs of superchunks gone cold, each with its job,
    /// being encoded by chunk storage's jobs, in the order taken.
    writing_back: VecDeque<(SuperchunkIndex, JobTicket)>,
    /// The superchunks whose changes were taken from the ring, each with
    /// its job, their images being rewritten by a job.
    flushing_superchunks: Vec<(SuperchunkIndex, JobTicket)>,
    /// The viewport last told, if the viewport's superchunks are hot.
    last_told_viewport: Option<Viewport>,
    /// Its superchunks within the world, sorted: wanted hot besides the
    /// halos.
    viewport: Vec<SuperchunkIndex>,
}

/// A superchunk warming: a halo reached it, and it turns hot at `due`.
#[derive(Clone, Copy, Debug)]
struct Warming {
    /// The superchunk.
    superchunk: SuperchunkIndex,
    /// The tick it turns hot at.
    due_tick: u64,
    /// Its job, building it whole.
    job: JobTicket,
}

impl Halos {
    /// No halos yet, as `hot` says which superchunks are to be, their
    /// jobs on `dispatcher`'s threads.
    pub(crate) fn new(hot: Hot, dispatcher: Arc<Dispatcher>) -> Self {
        Self { hot, jobs: Jobs::new(Arc::clone(&dispatcher)), warm_jobs: Jobs::new(dispatcher), warming: Vec::new(), cooling: Vec::new(), writing_back: VecDeque::new(), flushing_superchunks: Vec::new(), last_told_viewport: None, viewport: Vec::new() }
    }

    /// Takes `viewport` -- what the renderer renders now, if any --
    /// in place of the last: its superchunks within the world wanted
    /// hot from the next move on, if the world's hot says so
    /// ([`Hot::viewport`]); else nothing.
    pub(crate) fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        let viewport = viewport.filter(|_| self.hot.viewport());
        if viewport == self.last_told_viewport {
            return;
        }
        self.last_told_viewport = viewport;
        self.viewport.clear();
        if let Some(viewport) = viewport {
            let world_span_cartesian = self.hot.world_span_cartesian();
            let (viewport_columns_cartesian, viewport_rows_cartesian) = (viewport.top_left_superchunk_cartesian.0.max(world_span_cartesian.start)..=viewport.bottom_right_superchunk_cartesian.0.min(world_span_cartesian.end - 1), viewport.top_left_superchunk_cartesian.1.max(world_span_cartesian.start)..=viewport.bottom_right_superchunk_cartesian.1.min(world_span_cartesian.end - 1));
            self.viewport.extend(viewport_rows_cartesian.flat_map(|y| viewport_columns_cartesian.clone().map(move |x| SuperchunkIndex::from_cartesian(x, y))));
            self.viewport.sort_unstable();
        }
    }

    /// The viewport's superchunks within the world, sorted: none unless
    /// they are hot ([`Halos::keep_viewport`]).
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn viewport(&self) -> &[SuperchunkIndex] {
        &self.viewport
    }

    /// Takes up `cooling` -- sorted, each with the tick it goes cold
    /// at -- as the hot superchunks cooling: as a save kept them.
    pub(crate) fn restore_cooling(&mut self, cooling: Vec<(SuperchunkIndex, u64)>) {
        self.cooling = cooling;
    }

    /// Moves the halos to where the hot entities stand
    /// ([`Hot::wanted_hot_superchunks`]), with the viewport's superchunks: the superchunks
    /// reached warming, hot [`WARM_TICKS`] on; the rest cooling, cold
    /// [`COOL_TICKS`] on. Forced hot, it is the whole world all the while.
    pub(crate) fn move_to_hot_entities(&mut self, lent_world: &mut LentWorld<'_>) -> HaloChange {
        let mut wanted_superchunks = self.hot.wanted_hot_superchunks(lent_world.entity_store);
        if !self.viewport.is_empty() {
            wanted_superchunks.extend_from_slice(&self.viewport);
            wanted_superchunks.sort_unstable();
            wanted_superchunks.dedup();
        }
        self.make_hot_within(lent_world, &wanted_superchunks, WARM_TICKS, COOL_TICKS)
    }

    /// Makes `wanted_superchunks` -- sorted -- the hot superchunks now, nothing
    /// warming or cooling after: what generating and loading start
    /// from (`docs/simulation_reference.md`, "Halos::keep_hot").
    pub(crate) fn keep_hot(&mut self, lent_world: &mut LentWorld<'_>, wanted_superchunks: &[SuperchunkIndex]) -> HaloChange {
        let mut halo_change = self.make_hot_within(lent_world, wanted_superchunks, 0, 0);
        // One that was warming and is not wanted turned hot, as every warming does: cold now.
        if !self.cooling.is_empty() {
            halo_change += self.make_hot_within(lent_world, wanted_superchunks, 0, 0);
        }
        halo_change
    }

    /// The superchunks warming, each with the tick it turns hot at.
    pub fn warming(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.warming.iter().map(|warming| (warming.superchunk, warming.due_tick))
    }

    /// The hot superchunks cooling, each with the tick it goes cold at.
    pub fn cooling(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.cooling.iter().copied()
    }
}
