//! Compare-and-write and groups: a write applied only where its cell
//! is as its rule saw it, a group applied whole or not at all -- the
//! cell made to decay and be eaten in one tick, on purpose -- and both
//! the same on any number of threads.

use crate::tests::{cell, cells_drawn, walker, world, STONE};
use bitplane_manager::{BitmapArena, Write, WriteOp};
use coordinates::CellIndex;
use entity_manager::{Attribute, AttributeBlock, EntityId};
use simulation::{Simulation, Turn};
use utilities::rng::Rng;

/// An attribute an entity has once it has eaten.
const FED: Attribute<u64> = Attribute::new(43);
/// The place cells decayed are counted at.
const DECAYED: u32 = 3;
/// The place meals are counted at.
const EATEN: u32 = 4;

/// Sets `cells` of the stone, between two ticks.
fn set(arena: &mut BitmapArena, cells: &[CellIndex]) {
    cells.iter().for_each(|&at| arena.queue(STONE, Write::cell(at, WriteOp::Set)));
    assert_eq!(arena.apply().changed, cells.len() as u64);
}

/// Two writes on one cell that both saw it clear: the first applied
/// sets it, the second is refused. A write that says it saw what the
/// cell did not hold is refused, and nothing happens. Each counts only
/// if applied, under the number its rule's counts were given.
#[test]
fn a_write_is_applied_only_where_the_cell_is_as_it_was_seen() {
    let (mut arena, mut entities) = world(1);
    let (twice, never_set, cleared) = (cell(5, 5), cell(700, 9), cell(1023, 1023));
    set(&mut arena, &[cleared]);
    let report = Simulation::new(1).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        turn.count_under(10);
        turn.queue_seen(STONE, twice, 0, 1, Some(DECAYED));
        turn.queue_seen(STONE, twice, 0, 1, Some(DECAYED));
        turn.queue_seen(STONE, never_set, 1, 0, Some(EATEN));
        turn.queue_seen(STONE, cleared, 1, 0, None);
        0
    });
    let applied = report.writes_applied;
    assert_eq!((applied.writes, applied.changed, applied.refused, applied.missed), (4, 2, 2, 0));
    assert_eq!((arena.holds(STONE, twice), arena.holds(STONE, never_set), arena.holds(STONE, cleared)), (Ok(true), Ok(false), Ok(false)));
    assert_eq!((report.counted_when_applied[10 + DECAYED as usize], report.counted_when_applied[10 + EATEN as usize]), (1, 0));
    assert_eq!(report.counted_when_applied.iter().sum::<u64>(), 1);
}

/// A cell made to decay and be eaten in one tick, on purpose: two
/// eaters, each on a set cell, each queuing its meal as a group -- its
/// cell cleared, and what it comes to having eaten -- after a sleep
/// that stands if the meal does not; and the first one's cell cleared
/// by a write queued before the meals. That cell is cleared once and
/// the decay counted; its eater's meal is refused whole -- not fed,
/// not counted, asleep only a tick -- and the other's applied whole.
#[test]
fn a_cell_decayed_and_eaten_in_one_tick_is_a_meal_refused_whole() {
    let (mut arena, mut entities) = world(1);
    let (decaying, eaten) = (cell(300, 300), cell(301, 300));
    set(&mut arena, &[decaying, eaten]);
    entities.queue_put(walker(1, decaying, 0), &[]);
    entities.queue_put(walker(2, eaten, 0), &[]);
    entities.apply();
    let report = Simulation::new(1).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        turn.queue_seen(STONE, decaying, 1, 0, Some(DECAYED));
        for eater in turn.woken() {
            let (now, header) = (turn.now(), eater.header);
            turn.step(&header, header.at, now + 1);
            turn.group_start();
            turn.queue_seen(STONE, header.at, 1, 0, None);
            turn.put(entity_manager::Header { wake: now + 100, ..header }, &[AttributeBlock::holding(FED, 1)]);
            turn.count_if_applied(EATEN);
            turn.group_end();
        }
        0
    });
    assert_eq!((arena.holds(STONE, decaying), arena.holds(STONE, eaten)), (Ok(false), Ok(false)));
    let applied = report.writes_applied;
    assert_eq!((applied.writes, applied.changed, applied.refused), (3, 2, 1), "the meal on the cell decayed, refused");
    assert_eq!(report.groups, (1, 1), "a meal applied, a meal refused");
    assert_eq!((report.counted_when_applied[DECAYED as usize], report.counted_when_applied[EATEN as usize]), (1, 1));
    let (hungry, fed) = (entities.get(EntityId(1), decaying).expect("where it stood"), entities.get(EntityId(2), eaten).expect("where it stood"));
    assert_eq!((hungry.attribute(FED), hungry.header.wake), (None, 1), "nothing of its meal happened: asleep a tick");
    assert_eq!((fed.attribute(FED), fed.header.wake), (Some(1), 100), "its meal whole");
}

/// What a superchunk's turn queues in [`drawn_writes_and_groups`], for
/// the cells of `cells` in it, each as it was `seen`: for each, in
/// order, one to three times, a write that flips it -- or a group that
/// flips it and the next of its superchunk. Given to `queue` as the
/// cells flipped together.
fn attempts(cells: &[(CellIndex, bool)], superchunk: coordinates::SuperchunkIndex, mut queue: impl FnMut(&[(CellIndex, bool)])) {
    let here: Vec<(CellIndex, bool)> = cells.iter().copied().filter(|(at, _)| at.superchunk() == superchunk).collect();
    for (nth, &one) in here.iter().enumerate() {
        // Drawn from the cell, not the turn: the same whoever asks.
        let mut random = Rng::new(one.0 .0 ^ utilities::seed::counted());
        for _ in 0..random.between(1, 3) {
            match random.below(2) == 0 || here.len() < 2 {
                true => queue(&[one]),
                false => queue(&[one, here[(nth + 1) % here.len()]]),
            }
        }
    }
}

/// Cells drawn over four superchunks, some set, each written one to
/// three times in a tick, alone or in a group with another, every
/// write saying what the tick found: what comes of it is what
/// applying them one after another in the order queued comes to --
/// the first to change a cell applied, every later one refused, a
/// group refused whole -- cell by cell and count by count, on one
/// thread and on four.
#[test]
fn drawn_writes_and_groups_come_to_the_same_as_applied_in_order() {
    let mut random = Rng::new(utilities::seed::counted());
    let cells: Vec<(CellIndex, bool)> = cells_drawn(400, 2048).into_iter().map(|at| (at, random.below(2) == 0)).collect();
    // What applying them in order comes to.
    let (mut held, mut alone, mut together, mut refused, mut groups): (std::collections::HashMap<CellIndex, bool>, _, _, _, _) = (cells.iter().copied().collect(), 0u64, 0u64, 0u64, (0u64, 0u64));
    let mut superchunks: Vec<coordinates::SuperchunkIndex> = cells.iter().map(|(at, _)| at.superchunk()).collect();
    superchunks.sort_unstable();
    superchunks.dedup();
    for &superchunk in &superchunks {
        attempts(&cells, superchunk, |flipped| {
            let as_seen = flipped.iter().all(|(at, seen)| held[at] == *seen);
            match (as_seen, flipped.len()) {
                (true, 1) => alone += 1,
                (true, _) => (together, groups.0) = (together + 1, groups.0 + 1),
                (false, 1) => refused += 1,
                (false, _) => (refused, groups.1) = (refused + 2, groups.1 + 1),
            }
            if as_seen {
                flipped.iter().for_each(|(at, seen)| _ = held.insert(*at, !seen));
            }
        });
    }
    assert!(alone > 0 && together > 0 && refused > 0 && groups.1 > 0, "every case drawn");
    for threads in [1, 4] {
        let (mut arena, mut entities) = world(2);
        set(&mut arena, &cells.iter().filter(|(_, set)| *set).map(|(at, _)| *at).collect::<Vec<_>>());
        let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            let superchunk = turn.superchunk();
            attempts(&cells, superchunk, |flipped| match flipped {
                [(at, seen)] => turn.queue_seen(STONE, *at, u32::from(*seen), u32::from(!seen), Some(0)),
                _ => {
                    turn.group_start();
                    flipped.iter().for_each(|(at, seen)| turn.queue_seen(STONE, *at, u32::from(*seen), u32::from(!seen), None));
                    turn.count_if_applied(1);
                    turn.group_end();
                }
            });
            0
        });
        for (at, _) in &cells {
            assert_eq!(arena.holds(STONE, *at), Ok(held[at]), "on {threads} threads");
        }
        assert_eq!((report.counted_when_applied[0], report.counted_when_applied[1], report.writes_applied.refused, report.groups), (alone, together, refused, groups), "on {threads} threads");
    }
}
