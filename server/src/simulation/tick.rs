//! The tick, superchunk by superchunk, in two phases on the
//! dispatcher's threads: computing, every superchunk's rule reading
//! the world as the tick found it and queuing into its outbox; then
//! applying, each superchunk what was queued for it
//! (`docs/simulation.md`, "The tick" and "The dispatcher").

mod applying;

use crate::entity_manager::{WorldEntities, EntityReader, InstructionsApplied};
use crate::simulation::simulation_tick_state::conditional::{CountedWhenApplied, COUNTED_WHEN_APPLIED};
use crate::simulation::simulation_tick_state::{slots, Outbox, SimulationTickState};
use crate::bitplane_manager::{BitmapArena, CellReader, WritesApplied};
use utilities::coordinates::{CellIndex, SuperchunkIndex};
use std::ops::AddAssign;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use utilities::dispatcher::Dispatcher;
use utilities::rng::RandomSource;

/// A superchunk's turn to be claimed by a thread in the first phase:
/// its outbox and its random numbers.
type TurnToClaim<'a> = Mutex<(&'a mut Outbox, &'a mut (SuperchunkIndex, RandomSource))>;

/// What a tick did, and how long each phase took.
#[derive(Clone, Copy, Debug)]
pub struct TickReport<R> {
    /// What applying the writes did: a write landing in two superchunks
    /// counted in each.
    pub writes_applied: WritesApplied,
    /// What applying the instructions did.
    pub instructions_applied: InstructionsApplied,
    /// The compares come to as the tick was applied: those that
    /// held, and those that did not (`docs/simulation.md`,
    /// "Compare-and-write").
    pub instructions_compared: (u64, u64),
    /// What was counted as it was applied, by the number each rule's
    /// counts were given (`SimulationTickState::count_under`).
    pub counted_when_applied: CountedWhenApplied,
    /// The authors whose act reached over a border, each applied
    /// once to every superchunk it landed in
    /// (`docs/simulation.md`, "An act over a border").
    pub acts_over_a_border: u64,
    /// What the rule returned, added up over the superchunks.
    pub rules: R,
    /// The first phase's time: sampling and computing.
    pub computing_time: Duration,
    /// The second phase's time: applying.
    pub applying_time: Duration,
}

/// Ticks rules over an arena: the dispatcher's threads, and what each
/// tick reuses -- the outboxes, a superchunk each, and room for samples,
/// a part each -- so a tick allocates nothing once they have grown.
pub struct Simulation {
    /// The threads: its own, or shared with whatever else has work
    /// for them.
    dispatcher: Arc<Dispatcher>,
    /// The outboxes, a superchunk each, in the arena's order; emptied
    /// after every tick.
    outboxes: Vec<Outbox>,
    /// Room for samples, a part each.
    samples: Vec<Mutex<Vec<CellIndex>>>,
    /// Each superchunk's random stream, with its superchunk, in the
    /// arena's order: kept from tick to tick, and by a save.
    random_streams: Vec<(SuperchunkIndex, RandomSource)>,
    /// Room the second phase keeps from tick to tick.
    applying_room: applying::ApplyingRoom,
}

/// The threads `superchunks` superchunks are ticked on unless told
/// otherwise: every one the machine has -- threads are never held back
/// -- but no more than there are superchunks, a thread taking whole
/// superchunks.
pub fn threads_for(superchunks: usize) -> usize {
    std::thread::available_parallelism().map_or(1, usize::from).min(superchunks).max(1)
}

impl Simulation {
    /// A simulation of `superchunks` superchunks on every thread the
    /// machine has ([`threads_for`]), kept between ticks.
    pub fn for_superchunks(superchunks: usize) -> Self {
        Self::new(threads_for(superchunks))
    }

    /// A simulation on `thread_count` threads, kept between ticks: a number
    /// given only to measure against another.
    pub fn new(thread_count: usize) -> Self {
        Self::on_dispatcher(Arc::new(Dispatcher::new(thread_count)))
    }

    /// A simulation on `dispatcher`'s threads, which others may queue
    /// jobs on too: a thread busy with one sits a tick's phase out.
    pub fn on_dispatcher(dispatcher: Arc<Dispatcher>) -> Self {
        let samples = (0..dispatcher.thread_count()).map(|_| Mutex::new(Vec::new())).collect();
        Self { dispatcher, outboxes: Vec::new(), samples, random_streams: Vec::new(), applying_room: applying::ApplyingRoom::default() }
    }

    /// Each superchunk's random stream as it stands: its superchunk and
    /// its generator's state, in Morton order.
    pub fn random_states(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.random_streams.iter().map(|(superchunk_index, random_stream)| (*superchunk_index, random_stream.whole_state()))
    }

    /// Takes up `saved_states` as each superchunk's random stream --
    /// superchunk and state, sorted -- as a save kept them.
    pub fn restore_random(&mut self, saved_states: &[(SuperchunkIndex, u64)]) {
        debug_assert!(saved_states.is_sorted_by_key(|state_entry| state_entry.0));
        self.random_streams = saved_states.iter().map(|&(superchunk_index, stream_state)| (superchunk_index, RandomSource::new(stream_state))).collect();
    }

    /// Every superchunk of `superchunk_indices` given its random stream:
    /// the one it had, or a new one from `seed` and where it is.
    fn align_random(&mut self, superchunk_indices: &[SuperchunkIndex], seed: u64) {
        if self.random_streams.len() == superchunk_indices.len() && self.random_streams.iter().zip(superchunk_indices).all(|(kept_stream, &wanted_superchunk)| kept_stream.0 == wanted_superchunk) {
            return;
        }
        let had_streams = std::mem::take(&mut self.random_streams);
        self.random_streams = superchunk_indices
            .iter()
            .map(|&aligned_superchunk| match had_streams.binary_search_by_key(&aligned_superchunk, |had_stream| had_stream.0) {
                Ok(had_index) => (aligned_superchunk, RandomSource::new(had_streams[had_index].1.whole_state())),
                Err(_) => (aligned_superchunk, RandomSource::for_stream(seed, aligned_superchunk.0)),
            })
            .collect();
    }

    /// How many threads it ticks on.
    pub fn thread_count(&self) -> usize {
        self.dispatcher.thread_count()
    }

    /// One tick of `rule` over every superchunk of `bitmap_arena` and its
    /// entities: `rule` run on each, then what
    /// they queued applied. `seed`, the world's, seeds a superchunk's
    /// random numbers the first tick it is in.
    pub fn tick<R, F>(&mut self, bitmap_arena: &mut BitmapArena, entity_store: &mut WorldEntities, seed: u64, rule: F) -> TickReport<R>
    where
        R: Default + AddAssign + Send,
        F: Fn(&mut SimulationTickState) -> R + Sync,
    {
        let count = bitmap_arena.superchunks().len();
        self.outboxes.resize_with(count, Outbox::default);
        let parts = self.dispatcher.thread_count();

        let computing_start = Instant::now();
        let superchunk_indices = bitmap_arena.superchunk_indices();
        let mut instructions_applied = InstructionsApplied { lost: entity_store.keep_superchunks(&superchunk_indices), ..InstructionsApplied::default() };
        self.align_random(&superchunk_indices, seed);
        let current_tick = entity_store.current_tick();
        let superchunks = bitmap_arena.superchunks();
        let entity_superchunks = entity_store.superchunks();
        // The superchunks are not split among the threads beforehand: each thread claims the next not yet claimed, so none idles while another has work left.
        let turns: Vec<TurnToClaim> = self.outboxes.iter_mut().zip(&mut self.random_streams).map(Mutex::new).collect();
        let part_results: Vec<Mutex<R>> = (0..parts).map(|_| Mutex::new(R::default())).collect();
        let samples = &self.samples;
        let claimed_turns = AtomicUsize::new(0);
        self.dispatcher.run_in_parts(&|part| {
            let (bitmap_reader, entity_reader) = (CellReader::new(superchunks), EntityReader::new(entity_superchunks));
            let mut samples = samples[part].lock().expect("a part's samples");
            let mut part_total = R::default();
            loop {
                let turn_index = claimed_turns.fetch_add(1, Ordering::Relaxed);
                let Some(claimed_turn) = turns.get(turn_index) else {
                    break;
                };
                let (ref mut outbox, ref mut kept_random_stream) = *claimed_turn.lock().expect("a superchunk's turn");
                let mut tick_state = SimulationTickState::new(&superchunks[turn_index], &entity_superchunks[turn_index], current_tick, &bitmap_reader, &entity_reader, outbox, RandomSource::new(kept_random_stream.1.whole_state()), &mut samples);
                part_total += rule(&mut tick_state);
                debug_assert_eq!(tick_state.skip_tag_sets_open, 0, "a turn ended with a skip-tag set open");
                // Where its random numbers have come to: the next tick goes on from there.
                kept_random_stream.1 = tick_state.random_stream;
            }
            *part_results[part].lock().expect("a part's result") = part_total;
        });
        drop(turns);
        let mut rules = R::default();
        for part_result in part_results {
            rules += part_result.into_inner().expect("a part's result");
        }
        let computed_instant = Instant::now();

        let applied_parts = applying::apply_what_was_queued(&self.dispatcher, bitmap_arena.superchunks_mut(), entity_store.superchunks_mut(), &mut self.applying_room, &superchunk_indices, &self.outboxes, current_tick);
        // The tick over: every entity named by the cell it stands on now.
        for superchunk_entities in entity_store.superchunks_mut() {
            superchunk_entities.names_anew();
        }
        let (mut applied_writes, mut instructions_compared, mut counted_when_applied, mut acts_over_a_border) = (WritesApplied::default(), (0, 0), [0; COUNTED_WHEN_APPLIED], 0);
        for part in applied_parts {
            applied_writes += part.writes;
            acts_over_a_border += part.acts_over_a_border;
            instructions_applied += part.instructions;
            instructions_compared = (instructions_compared.0 + part.compares_held_and_not.0, instructions_compared.1 + part.compares_held_and_not.1);
            counted_when_applied.iter_mut().zip(part.counted_when_applied).for_each(|(count, part_count)| *count += part_count);
        }
        // Writes landing where no bitmap is in use are missed.
        for (source_outbox_index, outbox) in self.outboxes.iter_mut().enumerate() {
            for (slot, (dx, dy)) in slots() {
                let Some(target_superchunk) = superchunk_indices[source_outbox_index].at_cartesian_offset(dx, dy) else {
                    continue;
                };
                if superchunk_indices.binary_search(&target_superchunk).is_err() {
                    outbox.instruction_queues[slot].count_missed(&mut applied_writes, &mut instructions_applied);
                }
            }
            outbox.instruction_queues.iter_mut().for_each(|slot_queue| slot_queue.clear());
            outbox.author_segments.iter_mut().for_each(Vec::clear);
            outbox.across_acts.clear();
            outbox.named_by_others.clear();
        }
        entity_store.advance_tick();
        TickReport { writes_applied: applied_writes, instructions_applied, instructions_compared, counted_when_applied, acts_over_a_border, rules, computing_time: computed_instant - computing_start, applying_time: computed_instant.elapsed() }
    }
}
