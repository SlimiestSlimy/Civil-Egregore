//! Entities moving: across superchunk borders as whole copies, read
//! across superchunks as the tick found them, kept in Morton order by
//! cell however they step, and never two on a cell.
//!
//! `cargo test`

use crate::tests::*;
use coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{AttributeBlock, Entities, EntityId, EntityReader, Header, NEVER};
use simulation::{Simulation, Turn};
use std::sync::Mutex;

/// A turn reads entities in its neighbours, by ID and by chunk, as the
/// tick found them -- changes queued this tick unseen.
#[test]
fn turns_read_entities_across_superchunks() {
    let (mut arena, mut entities) = world(2);
    let (left, right) = (walker(1, cell(SUPERCHUNK_SIDE_CELLS - 1, 7), 0), walker(2, cell(SUPERCHUNK_SIDE_CELLS, 7), NEVER));
    entities.queue_put(left, &[]);
    entities.queue_put(right, &[]);
    entities.apply();
    let seen = Mutex::new(Vec::new());
    Simulation::new(2).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn, _| {
        for entity in turn.woken() {
            let neighbour = entity.header.at.offset(1, 0).unwrap();
            let by_id = turn.entity(EntityId(2), neighbour).map(|other| other.header);
            let in_chunk: Vec<_> = turn.entities_in(neighbour.chunk()).expect("hot").map(|other| other.header.id).collect();
            turn.remove(&entity.header);
            seen.lock().unwrap().push((by_id, in_chunk, turn.entity(entity.header.id, entity.header.at).is_some()));
        }
        0
    });
    assert_eq!(seen.into_inner().unwrap(), [(Some(right), vec![EntityId(2)], true)]);
    assert_eq!(entities.len(), 1);
    assert!(Entities::new().superchunk(SuperchunkIndex(0)).is_none());
}

/// A superchunk's entities wake in Morton order, by cell then ID,
/// however their wakes were filed: queued between ticks, or put in a
/// tick.
#[test]
fn entities_wake_in_morton_order() {
    let (mut arena, mut entities) = world(1);
    for (id, at) in (0..500u64).zip(cells_drawn(500, 1000)) {
        entities.queue_put(walker(1000 - id, at, 0), &[]);
    }
    entities.apply();
    let mut simulation = Simulation::new(1);
    for tick in 0..3 {
        let order = Mutex::new(Vec::new());
        simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn, _| {
            for entity in turn.woken() {
                order.lock().unwrap().push((entity.header.at, entity.header.id));
                let at = entity.header.at.offset(1, 1).unwrap();
                turn.update(&entity.header, Header { at, wake: turn.now() + 1, ..entity.header }, &[]);
            }
            0
        });
        let order = order.into_inner().unwrap();
        assert_eq!(order.len(), 500);
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "tick {tick}: woken out of Morton order");
    }
}

/// Entities stepping about their chunk and across its edges stay kept
/// in Morton order by cell, each found where it stands and nowhere
/// else, attributes its own.
#[test]
fn entities_stay_in_morton_order_as_they_step() {
    let (mut arena, mut entities) = world(1);
    for id in 0..600u64 {
        let at = cell(200 + (id % 20) as u32 * 3, 240 + (id / 20) as u32 * 3);
        entities.queue_put(walker(id + 1, at, 0), &[AttributeBlock::holding(WOKEN, id)]);
    }
    entities.apply();
    let mut simulation = Simulation::new(1);
    for tick in 0..200 {
        simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn, _| {
            for entity in turn.woken() {
                let (dx, dy) = (turn.random().below(5) as i32 - 2, turn.random().below(5) as i32 - 2);
                let after = Header { at: entity.header.at.offset(dx, dy).unwrap(), wake: turn.now() + 1, ..entity.header };
                turn.update(&entity.header, after, entity.attributes);
            }
            0
        });
        let kept: Vec<_> = entities.iter().map(|entity| (entity.header.at, entity.header.id)).collect();
        assert_eq!(kept.len(), 600);
        assert!(kept.windows(2).all(|pair| pair[0] < pair[1]), "tick {tick}: kept out of Morton order");
    }
    for entity in entities.iter() {
        let header = entity.header;
        assert_eq!(entity.attribute(WOKEN), Some(header.id.0 - 1), "its own attributes");
        assert_eq!(entities.get(header.id, header.at).map(|found| found.header), Some(header));
        assert!(entities.get(header.id, header.at.offset(1, 0).unwrap()).is_none(), "found only where it stands");
    }
}

/// A change to an entity no longer where the tick found it is passed
/// over: of two moving one entity in a tick, the first wins, and it
/// wakes once.
#[test]
fn a_change_to_an_entity_that_moved_on_is_passed_over() {
    let (mut arena, mut entities) = world(1);
    entities.queue_put(walker(1, cell(50, 50), 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn, _| {
        for entity in turn.woken() {
            for step in [1, 2] {
                let after = Header { at: entity.header.at.offset(step, 0).unwrap(), wake: turn.now() + 1, ..entity.header };
                turn.update(&entity.header, after, &[]);
            }
        }
        0
    });
    assert_eq!((report.instructions_applied.moves, report.instructions_applied.passed_over, entities.len()), (1, 1, 1));
    assert!(entities.get(EntityId(1), cell(51, 50)).is_some(), "the first move applied");
    let woken = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn, _| turn.woken().count()).rules;
    assert_eq!(woken, 1);
}

/// Entities never overlap: walkers crowded together, stepping at random
/// onto each other's cells, across chunk and superchunk borders, and
/// breeding onto cells drawn at random, stand one to a cell, and each on
/// one cell, after every tick -- and the cells a turn reads as stood on, about any cell and
/// across those borders, are the cells they stand on.
#[test]
fn entities_never_overlap() {
    let (mut arena, mut entities) = world(2);
    // Crowded about the corner the four superchunks meet at, which is the corner of chunks too.
    let corner = SUPERCHUNK_SIDE_CELLS - 20;
    for id in 0..900u64 {
        entities.queue_put(walker(id + 1, cell(corner + (id % 30) as u32, corner + (id / 30) as u32), id % 3), &[]);
    }
    // Two on a cell already taken: refused.
    entities.queue_put(walker(5000, cell(corner, corner), 0), &[]);
    entities.queue_put(walker(5001, cell(corner + 7, corner + 7), 0), &[]);
    let applied = entities.apply();
    assert_eq!((applied.puts, applied.refused, entities.len()), (900, 2, 900));
    let jostle = |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        for entity in turn.woken() {
            let header = entity.header;
            let (dx, dy) = (turn.random().below(3) as i32 - 1, turn.random().below(3) as i32 - 1);
            let wake = turn.now() + 1 + turn.random().below(3);
            turn.update(&header, Header { at: header.at.offset(dx, dy).unwrap(), wake, ..header }, entity.attributes);
            if turn.random().below(50) == 0 {
                let born = header.at.offset(turn.random().below(5) as i32 - 2, turn.random().below(5) as i32 - 2).unwrap();
                let id = turn.new_id();
                turn.put(Header { id, at: born, wake, ..header }, &[]);
            }
        }
        0
    };
    let mut simulation = Simulation::new(4);
    let (mut stayed, mut refused, mut crossed) = (0, 0, 0);
    for tick in 0..400 {
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), jostle);
        (stayed, refused) = (stayed + report.instructions_applied.stayed, refused + report.instructions_applied.refused);
        crossed += report.instructions_applied.crossed;
        let mut cells: Vec<CellIndex> = entities.iter().map(|entity| entity.header.at).collect();
        cells.sort_unstable();
        assert!(cells.windows(2).all(|pair| pair[0] != pair[1]), "tick {tick}: two entities on a cell");
        let mut ids: Vec<EntityId> = entities.iter().map(|entity| entity.header.id).collect();
        ids.sort_unstable();
        assert!(ids.windows(2).all(|pair| pair[0] != pair[1]), "tick {tick}: an entity on two cells");
        if tick % 50 == 0 {
            let reader = EntityReader::new(entities.superchunks());
            let mut random = utilities::rng::Rng::new(utilities::seed::counted().wrapping_add(tick));
            for _ in 0..40 {
                // Areas of every size, about the corner the superchunks meet at and off it.
                let (x, y) = (corner - 30 + random.below(90) as u32, corner - 30 + random.below(90) as u32);
                let (width, height) = (random.between(1, 16) as u32, random.between(1, 16) as u32);
                let rows = reader.occupied(cell(x, y), width, height);
                for (dx, dy) in (0..16).flat_map(|dy| (0..16).map(move |dx| (dx, dy))) {
                    let stood_on = dx < width && dy < height && cells.binary_search(&cell(x + dx, y + dy)).is_ok();
                    assert_eq!(rows[dy as usize] >> dx & 1 == 1, stood_on, "tick {tick}: ({dx}, {dy}) of the {width}x{height} cells from ({x}, {y})");
                }
            }
        }
    }
    assert!(stayed > 1000 && refused > 10 && crossed > 100, "{stayed} stayed, {refused} refused, {crossed} crossed");
    assert!(entities.len() > 900, "they bred");
}
