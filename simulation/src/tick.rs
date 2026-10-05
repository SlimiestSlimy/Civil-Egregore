//! The tick, superchunk by superchunk, in two phases, on the
//! dispatcher's threads (`../../docs/tilesim.md`, "The tick").
//!
//! 1. **Computing**: every superchunk runs the rule on itself -- samples
//!    its own cells, reads any cell in reach, and queues writes. Nothing
//!    changes in this phase, so every superchunk reads the world as the
//!    tick found it, and the threads share the arena read-only. A write
//!    is queued in its superchunk's outbox: nine queues, by where it
//!    lands -- the superchunk itself or one of its eight neighbours,
//!    never farther, the speed of light being a superchunk's side.
//! 2. **Applying**: every superchunk applies the writes queued for it --
//!    from its own outbox and its eight neighbours', in a fixed order --
//!    to its own bitmaps only. The threads share the outboxes
//!    read-only, and each changes only the superchunks it holds.
//!
//! Entities tick in the same two phases: in the first, the entities
//! waking in a superchunk run the rule with its cells, reading the world
//! as the tick found it, and queue instructions -- an entity put, moved,
//! edited or removed -- in the outbox slot of the superchunk each lands
//! in; in the second, each superchunk applies the instructions queued
//! for it, beside its writes. An entity moving to a
//! neighbour goes as a whole copy, made in the first phase.
//!
//! The threads hold contiguous runs of the superchunks, so each works
//! through them in Morton order, and the outboxes need no
//! synchronization: in the first phase each is written by its own
//! superchunk alone, in the second only read. Each superchunk has random
//! numbers of its own, kept from tick to tick, so a tick comes out the
//! same on any number of threads.

use crate::dispatcher::Dispatcher;
use crate::entity_store::{Entities, EntityId, EntityReader, Instructions, InstructionsApplied, SuperchunkEntities};
use crate::turn::{slot, Outbox, Turn};
use bitplane_manager::{count_missed, BitmapArena, Reader, Superchunk, WriteQueues, WritesApplied};
use coordinates::{CellIndex, SuperchunkIndex};
use std::ops::AddAssign;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use utilities::rng::Rng;

/// Superchunks a thread claims at a time, of a tick's work.
const CLAIMED: usize = 1;

/// A piece of the first phase, claimed by a thread: where its superchunks
/// start among them all, their outboxes, and their random numbers.
type PartOfTurns<'a> = (usize, &'a mut [Outbox], &'a mut [(SuperchunkIndex, Rng)]);

/// What a tick did, and how long each phase took.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickReport<R> {
    /// What applying the writes did: a write landing in two superchunks
    /// counted in each.
    pub writes_applied: WritesApplied,
    /// What applying the instructions did.
    pub instructions_applied: InstructionsApplied,
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
    /// The threads.
    dispatcher: Dispatcher,
    /// The outboxes, a superchunk each, in the arena's order; emptied
    /// after every tick.
    outboxes: Vec<Outbox>,
    /// Room for samples, a part each.
    samples: Vec<Mutex<Vec<CellIndex>>>,
    /// Each superchunk's random stream, with its superchunk, in the
    /// arena's order: kept from tick to tick, and by a save.
    random: Vec<(SuperchunkIndex, Rng)>,
    /// Each superchunk's entities crossed into it in a tick, with the
    /// cells they left, in the arena's order: emptied after every tick.
    arrived: Vec<Vec<(EntityId, CellIndex)>>,
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
        let dispatcher = Dispatcher::new(threads);
        let samples = (0..dispatcher.threads()).map(|_| Mutex::new(Vec::new())).collect();
        Self { dispatcher, outboxes: Vec::new(), samples, random: Vec::new(), arrived: Vec::new() }
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

    /// One tick of `rule`, over every superchunk of `arena` and its
    /// entities in `entities` -- made to hold the same superchunks: the
    /// first phase runs `rule` on each superchunk -- with room for
    /// samples -- and the second applies what they queued. `seed`, the
    /// world's, seeds a superchunk's random numbers the first tick it
    /// is in.
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
                    let mut turn = Turn { superchunk, entities: &entity_superchunks[first + offset], now, reader: &reader, entity_reader: &entity_reader, outbox, random };
                    total += rule(&mut turn, &mut samples);
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

        let (superchunk_indices, outboxes) = (&superchunk_indices, &self.outboxes);
        let superchunks: Vec<Mutex<(&mut [Superchunk], &mut [SuperchunkEntities])>> =
            arena.superchunks_mut().chunks_mut(per_part).zip(entities.superchunks_mut().chunks_mut(per_part)).map(Mutex::new).collect();
        let applied_parts: Vec<Mutex<(WritesApplied, InstructionsApplied)>> = (0..parts).map(|_| Mutex::new(Default::default())).collect();
        let claimed = AtomicUsize::new(0);
        self.dispatcher.run(&|part| {
            let (mut applied, mut instructions_applied) = (WritesApplied::default(), InstructionsApplied::default());
            while let Some(work) = superchunks.get(claimed.fetch_add(1, Ordering::Relaxed)) {
                let (ref mut superchunks, ref mut entity_superchunks) = *work.lock().expect("a piece's superchunks");
                for (superchunk, entities) in superchunks.iter_mut().zip(entity_superchunks.iter_mut()) {
                    let here = superchunk.index();
                    entities.pass(now);
                    for (dx, dy) in neighbours() {
                        let Some(source) = here.offset(dx, dy).and_then(|source| superchunk_indices.binary_search(&source).ok()) else {
                            continue;
                        };
                        let outbox = &outboxes[source];
                        for (layer_type, writes) in outbox.writes[slot(-dx, -dy)].iter() {
                            applied.writes += writes.len();
                            for &write in writes {
                                superchunk.apply(layer_type, write, &mut applied);
                            }
                        }
                        outbox.instructions[slot(-dx, -dy)].apply(std::slice::from_mut(entities), now + 1, &mut instructions_applied);
                    }
                    entities.sort_wakes(now + 1);
                }
            }
            *applied_parts[part].lock().expect("a part's result") = (applied, instructions_applied);
        });
        drop(superchunks);
        self.settle_crossings(entities, superchunk_indices, per_part);
        let mut applied = WritesApplied::default();
        for part in applied_parts {
            let (writes, instructions) = part.into_inner().expect("a part's result");
            applied += writes;
            instructions_applied += instructions;
        }
        // Writes landing where no bitmap is in use are missed.
        for (source, outbox) in self.outboxes.iter_mut().enumerate() {
            for (dx, dy) in neighbours() {
                let Some(target) = superchunk_indices[source].offset(dx, dy) else {
                    continue;
                };
                if superchunk_indices.binary_search(&target).is_err() {
                    for (_, writes) in outbox.writes[slot(dx, dy)].iter() {
                        applied.writes += writes.len();
                        writes.iter().for_each(|&write| count_missed(target, write, &mut applied));
                    }
                    outbox.instructions[slot(dx, dy)].count_lost(&mut instructions_applied);
                }
            }
            outbox.writes.iter_mut().for_each(WriteQueues::clear);
            outbox.instructions.iter_mut().for_each(Instructions::clear);
        }
        entities.advance();
        TickReport { writes_applied: applied, instructions_applied, rules, computing: computed - start, applying: computed.elapsed() }
    }

    /// Removes each entity that crossed into another superchunk this
    /// tick from the cell it left, each superchunk its own leavers
    /// ([`SuperchunkEntities::settle_leavers`]), on the threads, the
    /// superchunks split as for the second phase -- `per_part` a part.
    fn settle_crossings(&mut self, entities: &mut Entities, superchunk_indices: &[SuperchunkIndex], per_part: usize) {
        self.arrived.resize_with(superchunk_indices.len(), Vec::new);
        let mut crossed = false;
        for (superchunk, arrived) in entities.superchunks_mut().iter_mut().zip(&mut self.arrived) {
            superchunk.take_arrived(arrived);
            crossed |= !arrived.is_empty();
        }
        if crossed {
            let arrived = &self.arrived;
            let parts: Vec<Mutex<&mut [SuperchunkEntities]>> = entities.superchunks_mut().chunks_mut(per_part).map(Mutex::new).collect();
            let claimed = AtomicUsize::new(0);
            self.dispatcher.run(&|_| {
                while let Some(work) = parts.get(claimed.fetch_add(1, Ordering::Relaxed)) {
                    for superchunk in work.lock().expect("a piece's superchunks").iter_mut() {
                        for (dx, dy) in neighbours() {
                            if let Some(there) = superchunk.index().offset(dx, dy).and_then(|there| superchunk_indices.binary_search(&there).ok()) {
                                superchunk.settle_leavers(&arrived[there]);
                            }
                        }
                    }
                }
            });
            self.arrived.iter_mut().for_each(Vec::clear);
        }
    }
}

/// A superchunk's own place and its eight neighbours', as offsets, in a
/// fixed order: the order the second phase applies their writes in.
fn neighbours() -> impl Iterator<Item = (i32, i32)> {
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
}
