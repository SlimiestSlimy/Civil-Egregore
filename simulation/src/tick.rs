//! The tick, superchunk by superchunk, in two phases on the
//! dispatcher's threads: computing, every superchunk's rule reading
//! the world as the tick found it and queuing into its outbox; then
//! applying, each superchunk what was queued for it
//! (`docs/simulation.md`, "The tick" and "The dispatcher").

mod applying;

use entity_manager::{Entities, EntityReader, InstructionsApplied};
use crate::turn::apply::Standing;
use crate::turn::conditional::{CountedWhenApplied, COUNTED_WHEN_APPLIED};
use crate::turn::{slot, Outbox, Turn};
use bitplane_manager::{BitmapArena, Reader, WritesApplied};
use chunk_storage::LayerType;
use coordinates::{CellIndex, SuperchunkIndex};
use std::ops::AddAssign;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use utilities::dispatcher::Dispatcher;
use utilities::rng::Rng;

/// Superchunks a thread claims at a time, of a tick's work.
const CLAIMED: usize = 1;

/// A piece of the first phase, claimed by a thread: where its superchunks
/// start among them all, their outboxes, and their random numbers.
type PartOfTurns<'a> = (usize, &'a mut [Outbox], &'a mut [(SuperchunkIndex, Rng)]);

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
    /// counts were given (`Turn::count_under`).
    pub counted_when_applied: CountedWhenApplied,
    /// The authors whose act reached over a border, each applied
    /// once to every superchunk it landed in
    /// (`docs/simulation.md`, "An act over a border").
    pub acts_over_a_border: u64,
    /// What the rule returned, added up over the superchunks.
    pub rules: R,
    /// The first phase's time: sampling and computing.
    pub computing: Duration,
    /// The second phase's time: applying.
    pub applying: Duration,
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
    random: Vec<(SuperchunkIndex, Rng)>,
    /// The layer entities are kept in, if the world has one
    /// ([`Simulation::keep_entities_in`]).
    collision: Option<LayerType>,
    /// Room the second phase keeps from tick to tick.
    applying_room: applying::Room,
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

    /// A simulation on `threads` threads, kept between ticks: a number
    /// given only to measure against another.
    pub fn new(threads: usize) -> Self {
        Self::on(Arc::new(Dispatcher::new(threads)))
    }

    /// A simulation on `dispatcher`'s threads, which others may queue
    /// jobs on too: a thread busy with one sits a tick's phase out.
    pub fn on(dispatcher: Arc<Dispatcher>) -> Self {
        let samples = (0..dispatcher.threads()).map(|_| Mutex::new(Vec::new())).collect();
        Self { dispatcher, outboxes: Vec::new(), samples, random: Vec::new(), collision: None, applying_room: applying::Room::default() }
    }

    /// From here on the entities are kept in `collision_plane`, a
    /// layer of a bit a cell: the cell an entity comes to stand on is
    /// set there, none comes to a cell set there -- whatever set it --
    /// and the cell one left or was removed from is cleared when the
    /// tick is over (`docs/simulation.md`, "Entities").
    pub fn keep_entities_in(&mut self, collision_plane: LayerType) {
        self.collision = Some(collision_plane);
    }

    /// Each superchunk's random stream as it stands: its superchunk and
    /// its generator's state, in Morton order.
    pub fn random_states(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.random.iter().map(|(index, random)| (*index, random.state()))
    }

    /// Takes up `states` as each superchunk's random stream --
    /// superchunk and state, sorted -- as a save kept them.
    pub fn restore_random(&mut self, states: &[(SuperchunkIndex, u64)]) {
        debug_assert!(states.is_sorted_by_key(|state| state.0));
        self.random = states.iter().map(|&(index, state)| (index, Rng::new(state))).collect();
    }

    /// Every superchunk of `superchunk_indices` given its random stream:
    /// the one it had, or a new one from `seed` and where it is.
    fn align_random(&mut self, superchunk_indices: &[SuperchunkIndex], seed: u64) {
        if self.random.len() == superchunk_indices.len() && self.random.iter().zip(superchunk_indices).all(|(kept, &index)| kept.0 == index) {
            return;
        }
        let had = std::mem::take(&mut self.random);
        self.random = superchunk_indices
            .iter()
            .map(|&index| match had.binary_search_by_key(&index, |kept| kept.0) {
                Ok(at) => (index, Rng::new(had[at].1.state())),
                Err(_) => (index, Rng::for_stream(seed, index.0)),
            })
            .collect();
    }

    /// How many threads it ticks on.
    pub fn threads(&self) -> usize {
        self.dispatcher.threads()
    }

    /// One tick of `rule` over every superchunk of `arena` and its
    /// entities: `rule` run on each, with room for samples, then what
    /// they queued applied. `seed`, the world's, seeds a superchunk's
    /// random numbers the first tick it is in.
    pub fn tick<R, F>(&mut self, arena: &mut BitmapArena, entities: &mut Entities, seed: u64, rule: F) -> TickReport<R>
    where
        R: Default + AddAssign + Send,
        F: Fn(&mut Turn, &mut Vec<CellIndex>) -> R + Sync,
    {
        let count = arena.superchunks().len();
        self.outboxes.resize_with(count, Outbox::default);
        let parts = self.dispatcher.threads();
        // The superchunks are not split among the threads beforehand: each thread claims the next piece not yet claimed, so none idles while another has work left.
        let per_part = CLAIMED;

        let start = Instant::now();
        let superchunk_indices = arena.superchunk_indices();
        let mut instructions_applied = InstructionsApplied { lost: entities.align(&superchunk_indices), ..InstructionsApplied::default() };
        self.align_random(&superchunk_indices, seed);
        let now = entities.now();
        let superchunks = arena.superchunks();
        let entity_superchunks = entities.superchunks();
        let outboxes: Vec<Mutex<PartOfTurns>> = self
            .outboxes
            .chunks_mut(per_part)
            .zip(self.random.chunks_mut(per_part))
            .enumerate()
            .map(|(part, (outboxes, random))| Mutex::new((part * per_part, outboxes, random)))
            .collect();
        let results: Vec<Mutex<R>> = (0..parts).map(|_| Mutex::new(R::default())).collect();
        let samples = &self.samples;
        let claimed = AtomicUsize::new(0);
        self.dispatcher.run(&|part| {
            let (reader, entity_reader) = (Reader::new(superchunks), EntityReader::new(entity_superchunks));
            let mut samples = samples[part].lock().expect("a part's samples");
            let mut total = R::default();
            while let Some(work) = outboxes.get(claimed.fetch_add(1, Ordering::Relaxed)) {
                let (first, ref mut outboxes, ref mut random) = *work.lock().expect("a piece's outboxes");
                for (offset, (outbox, kept)) in outboxes.iter_mut().zip(random.iter_mut()).enumerate() {
                    let superchunk = &superchunks[first + offset];
                    let random = Rng::new(kept.1.state());
                    let mut turn = Turn { superchunk, entities: &entity_superchunks[first + offset], now, reader: &reader, entity_reader: &entity_reader, outbox, random, author: 0, comparing: None, compares_open: [crate::turn::conditional::CompareOpen::NONE; crate::turn::conditional::COMPARES_WITHIN], compares_open_count: 0, writes_queued: 0, counted_from: 0, across: false, author_began: None, landed_last: (crate::turn::NO_SUPERCHUNK, 0, 0) };
                    total += rule(&mut turn, &mut samples);
                    turn.compare_end();
                    // Where its random numbers have come to: the next tick goes on from there.
                    kept.1 = turn.random;
                }
            }
            *results[part].lock().expect("a part's result") = total;
        });
        drop(outboxes);
        let mut rules = R::default();
        for result in results {
            rules += result.into_inner().expect("a part's result");
        }
        let computed = Instant::now();

        let (superchunk_indices, collision) = (&superchunk_indices, self.collision);
        let applied_parts = applying::apply_what_was_queued(&self.dispatcher, arena.superchunks_mut(), entities.superchunks_mut(), &mut self.applying_room, superchunk_indices, &self.outboxes, now, collision);
        // The tick over: every entity named by the cell it stands on now, the cells left cleared in the collision plane.
        for (superchunk, entities) in arena.superchunks_mut().iter_mut().zip(entities.superchunks_mut()) {
            entities.names_anew(&mut Standing { superchunk, layer: collision });
        }
        let (mut applied, mut instructions_compared, mut counted_when_applied, mut acts_over_a_border) = (WritesApplied::default(), (0, 0), [0; COUNTED_WHEN_APPLIED], 0);
        for part in applied_parts {
            applied += part.writes;
            acts_over_a_border += part.acts_over_a_border;
            instructions_applied += part.instructions;
            instructions_compared = (instructions_compared.0 + part.compared.0, instructions_compared.1 + part.compared.1);
            counted_when_applied.iter_mut().zip(part.counted).for_each(|(count, more)| *count += more);
        }
        // Writes landing where no bitmap is in use are missed.
        for (source, outbox) in self.outboxes.iter_mut().enumerate() {
            for (dx, dy) in neighbours() {
                let Some(target) = superchunk_indices[source].offset(dx, dy) else {
                    continue;
                };
                if superchunk_indices.binary_search(&target).is_err() {
                    outbox.queues[slot(dx, dy)].count_missed(&mut applied, &mut instructions_applied);
                }
            }
            outbox.queues.iter_mut().for_each(|queue| queue.clear());
            outbox.segments.iter_mut().for_each(Vec::clear);
            outbox.across.clear();
            outbox.named_by_others.clear();
        }
        entities.advance();
        TickReport { writes_applied: applied, instructions_applied, instructions_compared, counted_when_applied, acts_over_a_border, rules, computing: computed - start, applying: computed.elapsed() }
    }
}

/// A superchunk's own place and its eight neighbours', as offsets, in a
/// fixed order: the order the second phase applies their writes in.
fn neighbours() -> impl Iterator<Item = (i32, i32)> {
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
}
