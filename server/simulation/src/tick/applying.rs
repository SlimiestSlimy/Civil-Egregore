//! A tick's second phase: each superchunk applies what was queued
//! for it, author by author in the one order of the whole world, on
//! the dispatcher's threads. An author whose act reaches over a
//! border is applied once, in the order it queued, when every
//! superchunk it lands in has come to it
//! (`docs/simulation.md`, "An act over a border").

use crate::simulation_tick_state::apply::PartApplied;
use crate::simulation_tick_state::landing::{LandingInOne, LandingInSeveral};
use crate::simulation_tick_state::{slot_toward, slot_between, slots, Outbox, ACROSS_QUEUE, SLOTS};
use bitplane_manager::Superchunk;
use coordinates::SuperchunkIndex;
use entity_manager::SuperchunkEntities;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use utilities::dispatcher::Dispatcher;
use following::EntityFollowing;
use who_goes_on::{LetGoOn, LONGEST_REST};

mod following;
mod who_goes_on;

/// One author's segment queued for a superchunk: the author, whose
/// outbox, which queue of it, which segment.
type QueuedSegment = (u64, u32, u32, u32);

/// A superchunk as it applies: itself, its entities, what is queued
/// for it in the order it is applied in, and how far it has come. A
/// line of the cache to itself, each being written to by its own
/// thread.
#[repr(align(64))]
struct ApplyingSuperchunk<'a> {
    /// The superchunk.
    superchunk: &'a mut Superchunk,
    /// Its entities.
    superchunk_entities: &'a mut SuperchunkEntities,
    /// What is queued for it, in order: gathered as it begins.
    segment_order: &'a mut Vec<QueuedSegment>,
    /// The next of those to apply.
    next_segment: usize,
    /// Whether it has begun.
    has_begun: bool,
}

/// What every thread of the second phase is given.
struct Phase<'a, 'b> {
    /// Every superchunk as it applies, in the arena's order: held by
    /// whoever applies to it.
    applying_superchunks: &'a [Mutex<ApplyingSuperchunk<'b>>],
    /// The superchunks, in that order.
    superchunk_indices: &'a [SuperchunkIndex],
    /// Their outboxes, in that order.
    outboxes: &'a [Outbox],
    /// The superchunks to begin with: all of them.
    to_go_on: &'a Mutex<Vec<usize>>,
    /// The superchunks that waited at an act over a border and can go
    /// on: a list a thread, of those it applied the act to and is at
    /// once back for, they being in its cache; and a last list, of
    /// those that are anyone's.
    let_go_on: &'a [LetGoOn],
    /// Whether each superchunk lies to be taken up: not begun, or let
    /// go on after an act over a border. Whoever makes it false has
    /// it; the lists only say where to look.
    lying_to_take_up: &'a [AtomicBool],
    /// Whose each superchunk is: the thread applying to it, or the
    /// list it lies in -- a thread's by the thread's number, anyone's
    /// by the number past them. Read to tell where a thread's own had
    /// best go, nothing more: it may be a moment behind.
    claimed_by: &'a [AtomicUsize],
    /// Held by a thread that goes to sleep with nothing to do, and by
    /// one that wakes it.
    asleep_lock: Mutex<()>,
    /// What a thread asleep is woken by.
    wake_signal: Condvar,
    /// How many threads are asleep or going to sleep.
    sleeping_threads: AtomicUsize,
    /// How many are not begun: read without the lock.
    to_begin: AtomicUsize,
    /// Whether any act over a border was queued this tick: if none,
    /// no superchunk waits for another, and a thread is done when
    /// none is left to begin.
    any_acts: bool,
    /// How many superchunks are done: written to once a superchunk,
    /// not at every act, being every thread's to read.
    superchunks_done: AtomicUsize,
    /// The tick running.
    current_tick: u64,
    /// What has the instructions naming an entity follow it over a
    /// border.
    entity_following: EntityFollowing,
}

impl Phase<'_, '_> {
    /// Gathers into `segment_order` what is queued for the superchunk at
    /// `here_arena_index`, in the order it is applied in: author by author in the
    /// Morton order of their cells, whichever superchunk each is in
    /// -- the same order in every superchunk, which is what lets one
    /// wait for another. Of one author, what it queued for the
    /// superchunk alone before its act over a border.
    fn gather_order(&self, here_arena_index: usize, segment_order: &mut Vec<QueuedSegment>) {
        segment_order.clear();
        for (_, (dx, dy)) in slots() {
            let Some(source_arena_index) = self.superchunk_indices[here_arena_index].at_cartesian_offset(dx, dy).and_then(|source_superchunk| self.superchunk_indices.binary_search(&source_superchunk).ok()) else {
                continue;
            };
            let (outbox, from_slot) = (&self.outboxes[source_arena_index], slot_toward(-dx, -dy));
            segment_order.extend(outbox.author_segments[from_slot].iter().enumerate().map(|(segment_number, segment_start)| (segment_start.author, source_arena_index as u32, from_slot as u32, segment_number as u32)));
            let across_landing_here = outbox.author_segments[ACROSS_QUEUE].iter().zip(&outbox.across_acts).enumerate().filter(|(_, (_, across_act))| across_act.lands_in_slots & 1 << from_slot != 0);
            segment_order.extend(across_landing_here.map(|(across_segment_number, (across_segment_start, _))| (across_segment_start.author, source_arena_index as u32, ACROSS_QUEUE as u32, across_segment_number as u32)));
        }
        // The sort keeps the order one queued in, and its sources' order: the same wherever it is gathered.
        segment_order.sort_by_key(|queued_segment| queued_segment.0);
    }

    /// Where each superchunk an act lands in is among them all, by
    /// the slot of it in the outbox of the superchunk at `source_arena_index`:
    /// those of `landing_slots` that are hot.
    fn landed_in(&self, source_arena_index: usize, landing_slots: u16) -> [Option<usize>; SLOTS] {
        let mut landed_arena_indices = [None; SLOTS];
        for (slot, (dx, dy)) in slots().filter(|(slot, _)| landing_slots & 1 << slot != 0) {
            landed_arena_indices[slot] = self.superchunk_indices[source_arena_index].at_cartesian_offset(dx, dy).and_then(|landed_superchunk| self.superchunk_indices.binary_search(&landed_superchunk).ok());
        }
        landed_arena_indices
    }

    /// Goes on applying to the superchunk at `here_arena_index`: every segment
    /// queued for it, in order, until it is done -- its wakes then
    /// sorted -- or it comes to an act over a border that another
    /// superchunk it lands in has not come to yet, and waits there,
    /// let go of. The last to come to such an act applies it, to all
    /// it lands in, and has the others go on: on the thread `part`
    /// first. If it waits, the superchunks it may be waiting for; if
    /// it is done, none.
    fn go_on(&self, here_arena_index: usize, part: usize, part_applied: &mut PartApplied) -> Vec<usize> {
        let mut applying_guard = self.applying_superchunks[here_arena_index].lock().expect("a superchunk applying");
        let here_applying = &mut *applying_guard;
        if !here_applying.has_begun {
            here_applying.has_begun = true;
            here_applying.superchunk_entities.pass_tick(self.current_tick);
            self.gather_order(here_arena_index, here_applying.segment_order);
        }
        while let Some(&(_, source_arena_index, queued_from, queued_segment)) = here_applying.segment_order.get(here_applying.next_segment) {
            let (outbox, from_slot, segment_number) = (&self.outboxes[source_arena_index as usize], queued_from as usize, queued_segment as usize);
            let segment_queue = &outbox.instruction_queues[from_slot];
            let (segment_start, next_segment_start) = (outbox.author_segments[from_slot][segment_number], outbox.author_segments[from_slot].get(segment_number + 1));
            let segment_instructions = segment_start.instructions as usize..next_segment_start.map_or(segment_queue.instructions.len(), |next_start| next_start.instructions as usize);
            let repointed_segment = self.entity_following.repointed_of(source_arena_index, from_slot as u32, segment_number as u32);
            if from_slot != ACROSS_QUEUE && repointed_segment.is_none() {
                self.give_up_those_kept(part);
                segment_queue.apply(segment_instructions, &mut LandingInOne { superchunk: here_applying.superchunk, superchunk_entities: here_applying.superchunk_entities }, self.current_tick + 1, part_applied);
                here_applying.next_segment += 1;
                continue;
            }
            // An act: of the queue of acts over a border, or a segment an entity that crossed had follow it.
            let arrived_so_far = match &repointed_segment {
                Some(segment_repointed) if from_slot != ACROSS_QUEUE => &*segment_repointed.arrived_count,
                _ => &outbox.across_acts[segment_number].arrived_count,
            };
            let arrived_with_this = usize::from(arrived_so_far.fetch_add(1, Ordering::SeqCst)) + 1;
            // Every superchunk it lands in, by where each is among them all: those its outbox says, then those an entity it names
            // crossed to -- looked up once counted, so the last to come sees every place another that came before was counted with.
            let repointed_segment = self.entity_following.repointed_of(source_arena_index, from_slot as u32, segment_number as u32);
            let mut places: Vec<usize> = if from_slot == ACROSS_QUEUE { self.landed_in(source_arena_index as usize, outbox.across_acts[segment_number].lands_in_slots).into_iter().flatten().collect() } else { Vec::new() };
            for &also_arena_index in repointed_segment.iter().flat_map(|repointed_also| &repointed_also.also_lands_in) {
                if !places.contains(&also_arena_index) {
                    places.push(also_arena_index);
                }
            }
            if arrived_with_this < places.len() {
                // Another it lands in is not here yet: whoever comes last applies it and has this one go on.
                places.retain(|&other_arena_index| other_arena_index != here_arena_index);
                return places;
            }
            // The last to come: the others wait at it, let go of, and are held here while it is applied.
            let mut others_locked: Vec<(usize, MutexGuard<ApplyingSuperchunk>)> = places.iter().filter(|&&filtered_arena_index| filtered_arena_index != here_arena_index).map(|&locked_arena_index| (locked_arena_index, self.applying_superchunks[locked_arena_index].lock().expect("a superchunk applying"))).collect();
            // Each held by its slot, if it is a neighbour of the outbox's superchunk, as an instruction for it says it; besides, if not:
            // a superchunk an entity the segment names crossed to.
            let source_index = self.superchunk_indices[source_arena_index as usize];
            let mut landing_in_several = LandingInSeveral { source_superchunk: source_index, slot_superchunks: Default::default(), also_superchunks: Vec::new(), crossed_entities: Vec::new() };
            let others_landing = others_locked.iter_mut().map(|(other_index, other_guard)| {
                let other_applying = &mut **other_guard;
                (*other_index, (&mut *other_applying.superchunk, &mut *other_applying.superchunk_entities))
            });
            for (place, landing_superchunk) in others_landing.chain([(here_arena_index, (&mut *here_applying.superchunk, &mut *here_applying.superchunk_entities))]) {
                match slot_between(source_index, self.superchunk_indices[place]) {
                    Some(slot) => landing_in_several.slot_superchunks[slot] = Some(landing_superchunk),
                    None => landing_in_several.also_superchunks.push(landing_superchunk),
                }
            }
            segment_queue.apply(segment_instructions, &mut landing_in_several, self.current_tick + 1, part_applied);
            let crossed_entities = std::mem::take(&mut landing_in_several.crossed_entities);
            drop(landing_in_several);
            part_applied.acts_over_a_border += u64::from(places.len() > 1);
            if !crossed_entities.is_empty() && !self.entity_following.nobody_named_another() {
                // Each superchunk held, by where it is among them all: what an entity that crossed has follow it is in their orders.
                let mut locked_applying: Vec<(usize, &mut ApplyingSuperchunk)> = others_locked.iter_mut().map(|(locked_other_index, locked_guard)| (*locked_other_index, &mut **locked_guard)).collect();
                locked_applying.push((here_arena_index, &mut *here_applying));
                self.have_follow(&crossed_entities, &mut locked_applying);
            }
            let at_another = here_applying.segment_order.get(here_applying.next_segment + 1).is_some_and(|next_queued| next_queued.2 == ACROSS_QUEUE as u32);
            for (released_arena_index, mut other_applying_guard) in others_locked {
                other_applying_guard.next_segment += 1;
                drop(other_applying_guard);
                // Kept for this thread if it is at once at another such act itself, and so back for it; else anyone's.
                self.let_lie(released_arena_index, if at_another { part } else { self.let_go_on.len() - 1 });
                if !at_another {
                    self.wake(false);
                }
            }
            here_applying.next_segment += 1;
        }
        here_applying.superchunk_entities.sort_wakes(self.current_tick + 1);
        if self.superchunks_done.fetch_add(1, Ordering::SeqCst) + 1 == self.applying_superchunks.len() {
            self.wake(true);
        }
        Vec::new()
    }
}

/// Room the second phase keeps from tick to tick, so that a tick
/// with little to apply does not spend itself asking for memory.
#[derive(Default)]
pub(crate) struct ApplyingRoom {
    /// What is queued for each superchunk, in order.
    segment_orders: Vec<Vec<QueuedSegment>>,
    /// [`Phase::lying_to_take_up`].
    lying_to_take_up: Vec<AtomicBool>,
    /// [`Phase::claimed_by`].
    claimed_by: Vec<AtomicUsize>,
    /// [`Phase::let_go_on`]: a list a thread, and one more that is
    /// anyone's.
    let_go_on: Vec<LetGoOn>,
    /// [`Phase::to_go_on`].
    to_go_on: Mutex<Vec<usize>>,
}

/// Applies what `outboxes` hold to `superchunks` and their
/// `entities_by_superchunk`, each superchunk what was queued for it, on
/// `dispatcher`'s threads: what each thread applied. `applying_room` is kept
/// from tick to tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_what_was_queued(dispatcher: &Dispatcher, superchunks: &mut [Superchunk], entities_by_superchunk: &mut [SuperchunkEntities], applying_room: &mut ApplyingRoom, superchunk_indices: &[SuperchunkIndex], outboxes: &[Outbox], current_tick: u64) -> Vec<PartApplied> {
    let (count, anyones_list_index) = (superchunks.len(), dispatcher.thread_count());
    applying_room.segment_orders.resize_with(count, Vec::new);
    applying_room.lying_to_take_up.clear();
    applying_room.lying_to_take_up.resize_with(count, || AtomicBool::new(true));
    applying_room.claimed_by.clear();
    applying_room.claimed_by.resize_with(count, || AtomicUsize::new(anyones_list_index));
    applying_room.let_go_on.resize_with(anyones_list_index + 1, LetGoOn::default);
    applying_room.let_go_on.iter_mut().for_each(LetGoOn::empty_out);
    // Taken from the end: the first superchunk first.
    let to_go_on = applying_room.to_go_on.get_mut().expect("the superchunks to begin with");
    to_go_on.clear();
    to_go_on.extend((0..count).rev());
    let applying_superchunks: Vec<Mutex<ApplyingSuperchunk>> =
        superchunks.iter_mut().zip(entities_by_superchunk).zip(&mut applying_room.segment_orders).map(|((superchunk, superchunk_entities), segment_order)| Mutex::new(ApplyingSuperchunk { superchunk, superchunk_entities, segment_order, next_segment: 0, has_begun: false })).collect();
    let (asleep_lock, wake_signal, sleeping_threads) = (Mutex::new(()), Condvar::new(), AtomicUsize::new(0));
    let (to_begin, any_acts, superchunks_done) = (AtomicUsize::new(count), outboxes.iter().any(|outbox| !outbox.across_acts.is_empty()), AtomicUsize::new(0));
    let phase = Phase { applying_superchunks: &applying_superchunks, superchunk_indices, outboxes, to_go_on: &applying_room.to_go_on, let_go_on: &applying_room.let_go_on, lying_to_take_up: &applying_room.lying_to_take_up, claimed_by: &applying_room.claimed_by, asleep_lock, wake_signal, sleeping_threads, to_begin, any_acts, superchunks_done, current_tick, entity_following: EntityFollowing::of_what_was_queued(outboxes) };
    let applied_parts: Vec<Mutex<PartApplied>> = (0..dispatcher.thread_count()).map(|_| Mutex::new(PartApplied::default())).collect();
    dispatcher.run_in_parts(&|part| {
        let mut part_applied = PartApplied::default();
        let mut rest_spins = 1;
        loop {
            match phase.next_for(part) {
                Some(mut here_arena_index) => {
                    rest_spins = 1;
                    // What it waits for, if that lies to be taken up, this thread goes on with itself: nothing waits for a thread to wake.
                    while let Some(waited_for) = phase.after_waiting(phase.go_on(here_arena_index, part, &mut part_applied), part) {
                        here_arena_index = waited_for;
                    }
                }
                None if !phase.any_acts || phase.superchunks_done.load(Ordering::SeqCst) == applying_superchunks.len() => break,
                // It looks again after a rest, longer each time, and then sleeps.
                None if rest_spins < LONGEST_REST => {
                    (0..rest_spins).for_each(|_| std::hint::spin_loop());
                    rest_spins *= 2;
                }
                None => phase.sleep(part),
            }
        }
        *applied_parts[part].lock().expect("a part's result") = part_applied;
    });
    applied_parts.into_iter().map(|part| part.into_inner().expect("a part's result")).collect()
}
