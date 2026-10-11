//! A tick's second phase: each superchunk applies what was queued
//! for it, author by author in the one order of the whole world, on
//! the dispatcher's threads. An author whose act reaches over a
//! border is applied once, in the order it queued, when every
//! superchunk it lands in has come to it
//! (`docs/simulation.md`, "An act over a border").

use crate::turn::apply::Applied;
use crate::turn::landing::{LandingInOne, LandingInSeveral};
use crate::turn::{slot, slot_between, Outbox, ACROSS, SLOTS};
use bitplane_manager::Superchunk;
use chunk_storage::LayerType;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::SuperchunkEntities;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use utilities::dispatcher::Dispatcher;
use following::Repointed;
use who_goes_on::{LetGoOn, LONGEST_REST};

mod following;
mod who_goes_on;

/// One author's segment queued for a superchunk: the author, whose
/// outbox, which queue of it, which segment.
type Segmented = (u64, u32, u32, u32);

/// A superchunk as it applies: itself, its entities, what is queued
/// for it in the order it is applied in, and how far it has come. A
/// line of the cache to itself, each being written to by its own
/// thread.
#[repr(align(64))]
struct Applying<'a> {
    /// The superchunk.
    superchunk: &'a mut Superchunk,
    /// Its entities.
    entities: &'a mut SuperchunkEntities,
    /// What is queued for it, in order: gathered as it begins.
    order: &'a mut Vec<Segmented>,
    /// The next of those to apply.
    next: usize,
    /// Whether it has begun.
    begun: bool,
}

/// What every thread of the second phase is given.
struct Phase<'a, 'b> {
    /// Every superchunk as it applies, in the arena's order: held by
    /// whoever applies to it.
    applying: &'a [Mutex<Applying<'b>>],
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
    lying: &'a [AtomicBool],
    /// Whose each superchunk is: the thread applying to it, or the
    /// list it lies in -- a thread's by the thread's number, anyone's
    /// by the number past them. Read to tell where a thread's own had
    /// best go, nothing more: it may be a moment behind.
    whose: &'a [AtomicUsize],
    /// Held by a thread that goes to sleep with nothing to do, and by
    /// one that wakes it.
    asleep: Mutex<()>,
    /// What a thread asleep is woken by.
    woken: Condvar,
    /// How many threads are asleep or going to sleep.
    sleeping: AtomicUsize,
    /// How many are not begun: read without the lock.
    to_begin: AtomicUsize,
    /// Whether any act over a border was queued this tick: if none,
    /// no superchunk waits for another, and a thread is done when
    /// none is left to begin.
    any_acts: bool,
    /// How many superchunks are done: written to once a superchunk,
    /// not at every act, being every thread's to read.
    done: AtomicUsize,
    /// The tick running.
    now: u64,
    /// The layer entities are kept in, if the world has one.
    collision: Option<LayerType>,
    /// The entities an author that is not the entity itself named
    /// this tick, each with those authors: by the cell the tick found
    /// it on. What is looked at as an entity crosses a border.
    named_by_others: HashMap<CellIndex, Vec<u64>>,
    /// Whether any segment was repointed this tick: read before the
    /// map is.
    any_repointed: AtomicBool,
    /// The segments an entity that crossed a border had follow it, by
    /// outbox, queue and segment.
    repointed: Mutex<HashMap<(u32, u32, u32), Repointed>>,
}

/// A superchunk's own place and its eight neighbours', as offsets, in
/// the order of their outbox slots.
fn slots() -> impl Iterator<Item = (usize, (i32, i32))> {
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (slot(dx, dy), (dx, dy))))
}

impl Phase<'_, '_> {
    /// Gathers into `order` what is queued for the superchunk at
    /// `here`, in the order it is applied in: author by author in the
    /// Morton order of their cells, whichever superchunk each is in
    /// -- the same order in every superchunk, which is what lets one
    /// wait for another. Of one author, what it queued for the
    /// superchunk alone before its act over a border.
    fn gather(&self, here: usize, order: &mut Vec<Segmented>) {
        order.clear();
        for (_, (dx, dy)) in slots() {
            let Some(source) = self.superchunk_indices[here].offset(dx, dy).and_then(|source| self.superchunk_indices.binary_search(&source).ok()) else {
                continue;
            };
            let (outbox, from) = (&self.outboxes[source], slot(-dx, -dy));
            order.extend(outbox.segments[from].iter().enumerate().map(|(segment, begun)| (begun.author, source as u32, from as u32, segment as u32)));
            let across = outbox.segments[ACROSS].iter().zip(&outbox.across).enumerate().filter(|(_, (_, across))| across.lands & 1 << from != 0);
            order.extend(across.map(|(segment, (begun, _))| (begun.author, source as u32, ACROSS as u32, segment as u32)));
        }
        // The sort keeps the order one queued in, and its sources' order: the same wherever it is gathered.
        order.sort_by_key(|segment| segment.0);
    }

    /// Where each superchunk an act lands in is among them all, by
    /// the slot of it in the outbox of the superchunk at `source`:
    /// those of `lands` that are hot. And how many.
    fn landed_in(&self, source: usize, lands: u16) -> ([Option<usize>; SLOTS], usize) {
        let (mut landed, mut count) = ([None; SLOTS], 0);
        for (slot, (dx, dy)) in slots().filter(|(slot, _)| lands & 1 << slot != 0) {
            landed[slot] = self.superchunk_indices[source].offset(dx, dy).and_then(|index| self.superchunk_indices.binary_search(&index).ok());
            count += usize::from(landed[slot].is_some());
        }
        (landed, count)
    }

    /// Goes on applying to the superchunk at `here`: every segment
    /// queued for it, in order, until it is done -- its wakes then
    /// sorted -- or it comes to an act over a border that another
    /// superchunk it lands in has not come to yet, and waits there,
    /// let go of. The last to come to such an act applies it, to all
    /// it lands in, and has the others go on: on the thread `part`
    /// first. If it waits, the superchunks it may be waiting for; if
    /// it is done, none.
    fn go_on(&self, here: usize, part: usize, applied: &mut Applied) -> [Option<usize>; SLOTS] {
        let mut guard = self.applying[here].lock().expect("a superchunk applying");
        let applying = &mut *guard;
        if !applying.begun {
            applying.begun = true;
            applying.entities.pass(self.now);
            self.gather(here, applying.order);
        }
        while let Some(&(_, source, from, segment)) = applying.order.get(applying.next) {
            let (outbox, from, segment) = (&self.outboxes[source as usize], from as usize, segment as usize);
            let queue = &outbox.queues[from];
            let (begun, next) = (outbox.segments[from][segment], outbox.segments[from].get(segment + 1));
            let some = begun.instructions as usize..next.map_or(queue.len(), |next| next.instructions as usize);
            let repointed = self.repointed_of(source, from as u32, segment as u32);
            if from != ACROSS && repointed.is_none() {
                self.give_up_those_kept(part);
                queue.apply(some, begun.compares as usize, &mut LandingInOne { superchunk: applying.superchunk, entities: applying.entities }, self.now + 1, self.collision, applied);
                applying.next += 1;
                continue;
            }
            // An act: of the queue of acts over a border, or a segment an entity that crossed had follow it.
            let (landed, mut count) = if from == ACROSS { self.landed_in(source as usize, outbox.across[segment].lands) } else { ([None; SLOTS], 0) };
            let also: Vec<usize> = repointed.as_ref().map_or(Vec::new(), |repointed| repointed.also.iter().copied().filter(|also| !landed.contains(&Some(*also))).collect());
            count += also.len();
            let arrived = match &repointed {
                Some(repointed) if from != ACROSS => &*repointed.arrived,
                _ => &outbox.across[segment].arrived,
            };
            if usize::from(arrived.fetch_add(1, Ordering::SeqCst)) + 1 < count {
                // Another it lands in is not here yet: whoever comes last applies it and has this one go on.
                let mut waited_for = landed.map(|other| other.filter(|&other| other != here));
                for (&other, place) in also.iter().filter(|&&other| other != here).zip(waited_for.iter_mut().filter(|place| place.is_none())) {
                    *place = Some(other);
                }
                return waited_for;
            }
            // The last to come: the others wait at it, let go of, and are held here while it is applied.
            let mut others: [Option<MutexGuard<Applying>>; SLOTS] = Default::default();
            for (slot, &other) in landed.iter().enumerate() {
                if let Some(other) = other.filter(|&other| other != here) {
                    others[slot] = Some(self.applying[other].lock().expect("a superchunk applying"));
                }
            }
            let mut others_also: Vec<(usize, MutexGuard<Applying>)> = also.iter().filter(|&&other| other != here).map(|&other| (other, self.applying[other].lock().expect("a superchunk applying"))).collect();
            let mut lands: [Option<(&mut Superchunk, &mut SuperchunkEntities)>; SLOTS] = Default::default();
            let mut own = Some((&mut *applying.superchunk, &mut *applying.entities));
            for ((slot, land), other) in lands.iter_mut().enumerate().zip(&mut others) {
                *land = match other {
                    Some(other) => {
                        let other = &mut **other;
                        Some((&mut *other.superchunk, &mut *other.entities))
                    }
                    None if landed[slot] == Some(here) => own.take(),
                    None => None,
                };
            }
            // Those held besides: by its slot, each that is a neighbour of the outbox's superchunk, as an instruction for it says it.
            let source_index = self.superchunk_indices[source as usize];
            let mut held_also = Vec::new();
            let besides = others_also.iter_mut().map(|(other, guard)| {
                let guard = &mut **guard;
                (*other, (&mut *guard.superchunk, &mut *guard.entities))
            });
            // This one too, if no slot's: the superchunk an entity the segment names crossed to.
            for (other, held) in besides.chain(own.take().map(|own| (here, own))) {
                match slot_between(source_index, self.superchunk_indices[other]) {
                    Some(slot) if lands[slot].is_none() => lands[slot] = Some(held),
                    _ => held_also.push(held),
                }
            }
            let mut landing = LandingInSeveral { source: source_index, lands, also: held_also, crossed: Vec::new() };
            queue.apply(some, begun.compares as usize, &mut landing, self.now + 1, self.collision, applied);
            let crossed = std::mem::take(&mut landing.crossed);
            drop(landing);
            applied.acts_over_a_border += u64::from(count > 1);
            if !crossed.is_empty() && !self.named_by_others.is_empty() {
                // Each superchunk held, by where it is among them all: what an entity that crossed has follow it is in their orders.
                let mut held: Vec<(usize, &mut Applying)> = others.iter_mut().enumerate().filter_map(|(slot, other)| other.as_mut().map(|other| (landed[slot].expect("held, so landed in"), &mut **other))).collect();
                held.extend(others_also.iter_mut().map(|(other, guard)| (*other, &mut **guard)));
                held.push((here, &mut *applying));
                self.have_follow(&crossed, &mut held);
            }
            let at_another = applying.order.get(applying.next + 1).is_some_and(|next| next.2 == ACROSS as u32);
            let let_go = others.into_iter().enumerate().filter_map(|(slot, other)| other.map(|other| (landed[slot].expect("held, so landed in"), other))).chain(others_also);
            for (index, mut other) in let_go {
                other.next += 1;
                drop(other);
                // Kept for this thread if it is at once at another such act itself, and so back for it; else anyone's.
                self.let_lie(index, if at_another { part } else { self.let_go_on.len() - 1 });
                if !at_another {
                    self.wake(false);
                }
            }
            applying.next += 1;
        }
        applying.entities.sort_wakes(self.now + 1);
        if self.done.fetch_add(1, Ordering::SeqCst) + 1 == self.applying.len() {
            self.wake(true);
        }
        [None; SLOTS]
    }
}

/// Room the second phase keeps from tick to tick, so that a tick
/// with little to apply does not spend itself asking for memory.
#[derive(Default)]
pub(crate) struct Room {
    /// What is queued for each superchunk, in order.
    orders: Vec<Vec<Segmented>>,
    /// [`Phase::lying`].
    lying: Vec<AtomicBool>,
    /// [`Phase::whose`].
    whose: Vec<AtomicUsize>,
    /// [`Phase::let_go_on`]: a list a thread, and one more that is
    /// anyone's.
    let_go_on: Vec<LetGoOn>,
    /// [`Phase::to_go_on`].
    to_go_on: Mutex<Vec<usize>>,
}

/// Applies what `outboxes` hold to `superchunks` and their
/// `entities`, each superchunk what was queued for it, on
/// `dispatcher`'s threads: what each thread applied. `room` is kept
/// from tick to tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_what_was_queued(dispatcher: &Dispatcher, superchunks: &mut [Superchunk], entities: &mut [SuperchunkEntities], room: &mut Room, superchunk_indices: &[SuperchunkIndex], outboxes: &[Outbox], now: u64, collision: Option<LayerType>) -> Vec<Applied> {
    let (count, anyones) = (superchunks.len(), dispatcher.threads());
    room.orders.resize_with(count, Vec::new);
    room.lying.clear();
    room.lying.resize_with(count, || AtomicBool::new(true));
    room.whose.clear();
    room.whose.resize_with(count, || AtomicUsize::new(anyones));
    room.let_go_on.resize_with(anyones + 1, LetGoOn::default);
    room.let_go_on.iter_mut().for_each(LetGoOn::empty);
    // Taken from the end: the first superchunk first.
    let to_go_on = room.to_go_on.get_mut().expect("the superchunks to begin with");
    to_go_on.clear();
    to_go_on.extend((0..count).rev());
    let applying: Vec<Mutex<Applying>> =
        superchunks.iter_mut().zip(entities).zip(&mut room.orders).map(|((superchunk, entities), order)| Mutex::new(Applying { superchunk, entities, order, next: 0, begun: false })).collect();
    let (asleep, woken, sleeping) = (Mutex::new(()), Condvar::new(), AtomicUsize::new(0));
    let (to_begin, any_acts, done) = (AtomicUsize::new(count), outboxes.iter().any(|outbox| !outbox.across.is_empty()), AtomicUsize::new(0));
    let mut named_by_others: HashMap<CellIndex, Vec<u64>> = HashMap::new();
    for &(named, author) in outboxes.iter().flat_map(|outbox| &outbox.named_by_others) {
        named_by_others.entry(named).or_default().push(author);
    }
    let (any_repointed, repointed) = (AtomicBool::new(false), Mutex::new(HashMap::new()));
    let phase = Phase { applying: &applying, superchunk_indices, outboxes, to_go_on: &room.to_go_on, let_go_on: &room.let_go_on, lying: &room.lying, whose: &room.whose, asleep, woken, sleeping, to_begin, any_acts, done, now, collision, named_by_others, any_repointed, repointed };
    let applied_parts: Vec<Mutex<Applied>> = (0..dispatcher.threads()).map(|_| Mutex::new(Applied::default())).collect();
    dispatcher.run(&|part| {
        let mut applied = Applied::default();
        let mut rests = 1;
        loop {
            match phase.next_for(part) {
                Some(mut here) => {
                    rests = 1;
                    // What it waits for, if that lies to be taken up, this thread goes on with itself: nothing waits for a thread to wake.
                    while let Some(waited_for) = phase.after_waiting(phase.go_on(here, part, &mut applied), part) {
                        here = waited_for;
                    }
                }
                None if !phase.any_acts || phase.done.load(Ordering::SeqCst) == applying.len() => break,
                // It looks again after a rest, longer each time, and then sleeps.
                None if rests < LONGEST_REST => {
                    (0..rests).for_each(|_| std::hint::spin_loop());
                    rests *= 2;
                }
                None => phase.sleep(part),
            }
        }
        *applied_parts[part].lock().expect("a part's result") = applied;
    });
    applied_parts.into_iter().map(|part| part.into_inner().expect("a part's result")).collect()
}
