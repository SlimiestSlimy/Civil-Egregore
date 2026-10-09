//! Entities ticked: woken at their tick and no other, in Morton order;
//! attributes added and removed at run time; changes queued outside a
//! tick; and the same on any number of threads.
//!
//! `cargo test`

use crate::tests::*;
use coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{EntityId, Header, WHEEL_TICKS};
use simulation::{Simulation, Turn};
use std::sync::Mutex;

/// Attributes added and removed every tick, by hundreds of entities in
/// one chunk -- so the old runs pile up as garbage and are swept -- come
/// out right.
#[test]
fn attributes_come_and_go_at_run_time() {
    let (mut arena, mut entities) = world(1);
    for id in 0..300 {
        entities.queue_put(walker(id * 7919 + 1, cell(id as u32 % 200, 5 + id as u32 / 200), 0), &[]);
    }
    assert_eq!(entities.queued(), 300);
    assert_eq!(entities.apply().puts, 300);
    assert_eq!((entities.len(), entities.queued()), (300, 0));
    let mut simulation = Simulation::new(1);
    for tick in 1..=101u64 {
        let report = simulation.tick(&mut arena, &mut entities, tick, count_and_flip);
        assert_eq!((report.rules, report.instructions_applied.puts), (300, 300));
    }
    assert_eq!(entities.len(), 300);
    for entity in entities.iter() {
        assert_eq!(entity.attribute(WOKEN), Some(101));
        assert_eq!(entity.attribute(ODD), Some(1), "flipped 101 times");
        assert_eq!(entity.attributes.len(), 2);
    }
}

/// An entity wakes at its tick and no other: one in reach of the wheel,
/// one past it, and none once removed.
#[test]
fn entities_wake_at_their_tick() {
    let (mut arena, mut entities) = world(1);
    let far = 2 * WHEEL_TICKS + 300;
    entities.queue_put(walker(1, cell(3, 3), 5), &[]);
    entities.queue_put(walker(2, cell(900, 900), far), &[]);
    entities.queue_put(walker(3, cell(10, 10), 7), &[]);
    entities.apply();
    let woken = Mutex::new(Vec::new());
    let mut simulation = Simulation::new(1);
    for tick in 0..=far + 10 {
        simulation.tick(&mut arena, &mut entities, tick, |turn, _| {
            for entity in turn.woken() {
                woken.lock().unwrap().push((entity.header.id.0, turn.now()));
                if entity.header.id == EntityId(3) {
                    turn.remove(&entity.header);
                } else if entity.header.id == EntityId(1) && turn.now() == 5 {
                    turn.put(Header { wake: 40, ..entity.header }, entity.attributes);
                }
            }
            0
        });
    }
    assert_eq!(woken.into_inner().unwrap(), [(1, 5), (3, 7), (1, 40), (2, far)]);
    assert_eq!(entities.len(), 2, "the third removed");
}

/// Walkers stepping right a cell a tick cross into the next superchunk,
/// as whole copies, attributes and all -- the one left behind gone by
/// the tick's end -- and at the edge of the hot superchunks stay, lost
/// to nowhere.
#[test]
fn entities_cross_borders_and_stay_at_the_edge_of_the_hot_world() {
    let (mut arena, mut entities) = world(2);
    let start = SUPERCHUNK_SIDE_CELLS - 3;
    entities.queue_put(walker(9, cell(start, 100), 0), &[entity_manager::Attribute { kind: WOKEN, value: 0 }]);
    entities.apply();
    let mut simulation = Simulation::new(2);
    let step = |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        for entity in turn.woken() {
            let after = Header { at: entity.header.at.offset(1, 0).unwrap(), wake: turn.now() + 1, ..entity.header };
            turn.update(&entity.header, after, &[entity_manager::Attribute { kind: WOKEN, value: entity.attribute(WOKEN).unwrap() + 1 }]);
        }
        0
    };
    let mut most = 0;
    for tick in 0..10 {
        simulation.tick(&mut arena, &mut entities, tick, step);
        most = most.max(entities.len());
    }
    assert_eq!(most, 1, "never here and there at a tick's end");
    let right = SuperchunkIndex::from_cartesian(11, 10);
    let entity = entities.superchunk(right).and_then(|superchunk| superchunk.iter().next()).expect("in the right neighbour");
    assert_eq!((entity.header.at, entity.attribute(WOKEN)), (cell(start + 10, 100), Some(10)));
    for tick in 10..1100 {
        simulation.tick(&mut arena, &mut entities, tick, step);
    }
    let all: Vec<_> = entities.iter().map(|entity| entity.header.at).collect();
    assert_eq!(all, [cell(2 * SUPERCHUNK_SIDE_CELLS - 1, 100)], "at the edge of the hot superchunks, not lost past it");
}

/// Walkers wandering at random, giving birth and dying over 3x3
/// superchunks, across their borders: the same on one thread and four.
#[test]
fn any_number_of_threads_ticks_entities_the_same() {
    let wander = |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        let mut changes = 0;
        for entity in turn.woken() {
            let header = entity.header;
            let (dx, dy) = (turn.random().below(3) as i32 - 1, turn.random().below(3) as i32 - 1);
            let at = header.at.offset(dx * 7, dy * 7).unwrap();
            let wake = turn.now() + 1 + turn.random().below(5);
            match turn.random().below(40) {
                0 => turn.remove(&header),
                1 => {
                    let child = Header { id: turn.new_id(), at, wake, ..header };
                    turn.put(child, &[]);
                    turn.put(Header { wake: turn.now() + 3, ..header }, entity.attributes);
                }
                _ => turn.update(&header, Header { at, wake, ..header }, entity.attributes),
            }
            changes += 1;
        }
        changes
    };
    let run = |threads| {
        let (mut arena, mut entities) = world(3);
        for id in 0..2000u64 {
            entities.queue_put(walker(id + 1, cell(((id * 7919) % 3072) as u32, ((id * 104_729) % 3072) as u32), id % 4), &[]);
        }
        entities.apply();
        let mut simulation = Simulation::new(threads);
        let changes: usize = (0..300).map(|tick| simulation.tick(&mut arena, &mut entities, tick, wander).rules).sum();
        let all: Vec<(Header, Vec<_>)> = entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
        (changes, all)
    };
    let (one, four) = (run(1), run(4));
    assert!(one.0 > 10_000 && !one.1.is_empty());
    assert_eq!(one, four);
}

/// Changes queued outside a tick land where their cells are, and one
/// past the hot superchunks is lost; a removal takes its entity out.
#[test]
fn changes_outside_a_tick_are_queued_then_applied() {
    let (_, mut entities) = world(1);
    let (here, away) = (walker(1, cell(5, 5), 0), walker(2, cell(2 * SUPERCHUNK_SIDE_CELLS, 5), 0));
    entities.queue_put(here, &[]);
    entities.queue_put(away, &[]);
    assert_eq!(entities.len(), 0, "nothing changes until applied");
    let applied = entities.apply();
    assert_eq!((applied.puts, applied.lost, entities.len()), (1, 1, 1));
    assert_eq!(entities.get(here.id, here.at).map(|entity| entity.header), Some(here));
    entities.queue_remove(&here);
    assert_eq!((entities.apply().removes, entities.len()), (1, 0));
}
