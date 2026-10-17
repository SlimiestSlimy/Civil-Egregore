//! Entities moving: across superchunk borders as whole copies, read
//! across superchunks as the tick found them, kept in Morton order by
//! cell however they step, and never two on a cell.
//!
//! `cargo test`

use crate::arenas::*;
use crate::walkers::*;
use coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{AttributeBlock, WorldEntities, EntityReader, Header, NEVER_WAKES};
use simulation::{Simulation, Turn};
use std::sync::Mutex;

/// A turn reads entities in its neighbours, by cell and by chunk, as the
/// tick found them -- changes queued this tick unseen.
#[test]
fn turns_read_entities_across_superchunks() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(2);
    let (left_walker, right_walker) = (walker_header(cell(SUPERCHUNK_SIDE_CELLS - 1, 7), 0), walker_header(cell(SUPERCHUNK_SIDE_CELLS, 7), NEVER_WAKES));
    entity_store.queue_put(left_walker, &[]);
    entity_store.queue_put(right_walker, &[]);
    entity_store.apply();
    let seen_log = Mutex::new(Vec::new());
    Simulation::new(2).tick(&mut bitmap_arena, &mut entity_store, seed, |turn| {
        for entity in turn.woken_entities() {
            let east_neighbour = entity.header.stood_cell.at_cartesian_offset(1, 0).unwrap();
            let by_cell = turn.entity_on(east_neighbour).map(|found_entity| found_entity.header);
            let in_chunk: Vec<_> = turn.entities_in(east_neighbour.chunk()).expect("hot").map(|chunk_entity| chunk_entity.header.stood_cell).collect();
            actions::entities::remove(turn, &entity.header);
            seen_log.lock().unwrap().push((by_cell, in_chunk, turn.entity_on(entity.header.stood_cell).is_some()));
        }
        0
    });
    assert_eq!(seen_log.into_inner().unwrap(), [(Some(right_walker), vec![right_walker.stood_cell], true)]);
    assert_eq!(entity_store.len(), 1);
    assert!(WorldEntities::new().superchunk(SuperchunkIndex(0)).is_none());
}

/// A superchunk's entities wake in Morton order by cell,
/// however their wakes were filed: queued between ticks, or put in a
/// tick.
#[test]
fn entities_wake_in_morton_order() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    for walker_cell in cells_drawn(seed, 500, 1000) {
        entity_store.queue_put(walker_header(walker_cell, 0), &[]);
    }
    entity_store.apply();
    let mut world_simulation = Simulation::new(1);
    for tick in 0..3 {
        let woken_order = Mutex::new(Vec::new());
        world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn| {
            for entity in turn.woken_entities() {
                woken_order.lock().unwrap().push(entity.header.stood_cell);
                let stepped_cell = entity.header.stood_cell.at_cartesian_offset(1, 1).unwrap();
                actions::entities::update_entity(turn, &entity.header, Header { stood_cell: stepped_cell, wake: turn.current_tick() + 1, ..entity.header }, &[]);
            }
            0
        });
        let woken_cells = woken_order.into_inner().unwrap();
        assert_eq!(woken_cells.len(), 500);
        assert!(woken_cells.windows(2).all(|cell_pair| cell_pair[0] < cell_pair[1]), "tick {tick}: woken out of Morton order");
    }
}

/// Entities stepping about their chunk and across its edges stay kept
/// in Morton order by cell, each found where it stands and nowhere
/// else, attributes its own.
#[test]
fn entities_stay_in_morton_order_as_they_step() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    for walker_number in 0..600u64 {
        let walker_cell = cell(200 + (walker_number % 20) as u32 * 3, 240 + (walker_number / 20) as u32 * 3);
        entity_store.queue_put(walker_header(walker_cell, 0), &[AttributeBlock::holding_value(ENTITY_NUMBER, walker_number)]);
    }
    entity_store.apply();
    let mut world_simulation = Simulation::new(1);
    for tick in 0..200 {
        world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn| {
            for entity in turn.woken_entities() {
                let (dx, dy) = (turn.random_stream().draw_below(5) as i32 - 2, turn.random_stream().draw_below(5) as i32 - 2);
                let stepped_header = Header { stood_cell: entity.header.stood_cell.at_cartesian_offset(dx, dy).unwrap(), wake: turn.current_tick() + 1, ..entity.header };
                actions::entities::update_entity(turn, &entity.header, stepped_header, entity.attributes);
            }
            0
        });
        let kept_cells: Vec<_> = entity_store.iter().map(|entity| entity.header.stood_cell).collect();
        assert_eq!(kept_cells.len(), 600);
        assert!(kept_cells.windows(2).all(|cell_pair| cell_pair[0] < cell_pair[1]), "tick {tick}: kept out of Morton order");
    }
    for entity in entity_store.iter() {
        let header = entity.header;
        assert_eq!(entity_store.get(header.stood_cell).map(|found_entity| (found_entity.header, found_entity.attribute(ENTITY_NUMBER))), Some((header, entity.attribute(ENTITY_NUMBER))), "found where it stands");
    }
    let mut entity_numbers: Vec<_> = entity_store.iter().map(|entity| entity.attribute(ENTITY_NUMBER).expect("its own attributes")).collect();
    entity_numbers.sort_unstable();
    assert!(entity_numbers.into_iter().eq(0..600), "each its own attributes still, none lost and none twice");
}

/// A change to an entity no longer where the tick found it is passed
/// over: of two moving one entity in a tick, the first wins, and it
/// wakes once.
#[test]
fn a_change_to_an_entity_that_moved_on_is_passed_over() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    entity_store.queue_put(walker_header(cell(50, 50), 0), &[]);
    entity_store.apply();
    let mut world_simulation = Simulation::new(1);
    let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn| {
        for entity in turn.woken_entities() {
            for step in [1, 2] {
                let stepped_header = Header { stood_cell: entity.header.stood_cell.at_cartesian_offset(step, 0).unwrap(), wake: turn.current_tick() + 1, ..entity.header };
                actions::entities::update_entity(turn, &entity.header, stepped_header, &[]);
            }
        }
        0
    });
    assert_eq!((report.instructions_applied.moves, report.instructions_applied.passed_over, entity_store.len()), (1, 1, 1));
    assert!(entity_store.get(cell(51, 50)).is_some(), "the first move applied");
    let woken_count = world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn| turn.woken_entities().count()).rules;
    assert_eq!(woken_count, 1);
}

/// Entities never overlap: walkers crowded together, stepping at random
/// onto each other's cells, across chunk and superchunk borders, and
/// breeding onto cells drawn at random, stand one to a cell, and each on
/// one cell, after every tick -- and the cells a turn reads as stood on, about any cell and
/// across those borders, are the cells they stand on.
#[test]
fn entities_never_overlap() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut bitmap_arena, mut entity_store) = world(2);
    // Crowded about the corner the four superchunks meet at, which is the corner of chunks too.
    let crowd_corner = SUPERCHUNK_SIDE_CELLS - 20;
    for walker_number in 0..900u64 {
        entity_store.queue_put(walker_header(cell(crowd_corner + (walker_number % 30) as u32, crowd_corner + (walker_number / 30) as u32), walker_number % 3), &[AttributeBlock::holding_value(ENTITY_NUMBER, walker_number)]);
    }
    // Two on a cell already taken: refused.
    entity_store.queue_put(walker_header(cell(crowd_corner, crowd_corner), 0), &[]);
    entity_store.queue_put(walker_header(cell(crowd_corner + 7, crowd_corner + 7), 0), &[]);
    let applied_counts = entity_store.apply();
    assert_eq!((applied_counts.puts, applied_counts.puts_refused, entity_store.len()), (900, 2, 900));
    let jostle_rule = |turn: &mut Turn| {
        for entity in turn.woken_entities() {
            let header = entity.header;
            let (dx, dy) = (turn.random_stream().draw_below(3) as i32 - 1, turn.random_stream().draw_below(3) as i32 - 1);
            let wake = turn.current_tick() + 1 + turn.random_stream().draw_below(3);
            actions::entities::update_entity(turn, &header, Header { stood_cell: header.stood_cell.at_cartesian_offset(dx, dy).unwrap(), wake, ..header }, entity.attributes);
            if turn.random_stream().draw_below(50) == 0 {
                let born_cell = header.stood_cell.at_cartesian_offset(turn.random_stream().draw_below(5) as i32 - 2, turn.random_stream().draw_below(5) as i32 - 2).unwrap();
                let born_number = 1000 + turn.random_stream().next_draw();
                actions::entities::make_entity(turn, header.entity_type, born_cell, wake, &[AttributeBlock::holding_value(ENTITY_NUMBER, born_number)]);
            }
        }
        0
    };
    let mut world_simulation = Simulation::new(4);
    let (mut stayed_count, mut refused_count, mut crossed_count) = (0, 0, 0);
    for tick in 0..400 {
        let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, jostle_rule);
        (stayed_count, refused_count) = (stayed_count + report.instructions_applied.turned_back, refused_count + report.instructions_applied.puts_refused);
        crossed_count += report.instructions_applied.border_crossings;
        let mut cells: Vec<CellIndex> = entity_store.iter().map(|entity| entity.header.stood_cell).collect();
        cells.sort_unstable();
        assert!(cells.windows(2).all(|cell_pair| cell_pair[0] != cell_pair[1]), "tick {tick}: two entities on a cell");
        let mut entity_numbers: Vec<u64> = entity_store.iter().map(|entity| entity.attribute(ENTITY_NUMBER).expect("tagged")).collect();
        entity_numbers.sort_unstable();
        assert!(entity_numbers.windows(2).all(|number_pair| number_pair[0] != number_pair[1]), "tick {tick}: an entity on two cells");
        if tick % 50 == 0 {
            let entity_reader = EntityReader::new(entity_store.superchunks());
            let mut seeded_rng = utilities::rng::RandomSource::new(seed.wrapping_add(tick));
            for _ in 0..40 {
                // Areas of every size, about the corner the superchunks meet at and off it.
                let (x, y) = (crowd_corner - 30 + seeded_rng.draw_below(90) as u32, crowd_corner - 30 + seeded_rng.draw_below(90) as u32);
                let (width, height) = (seeded_rng.draw_between(1, 16) as u32, seeded_rng.draw_between(1, 16) as u32);
                let occupied_rows = entity_reader.occupied(cell(x, y), width, height);
                for (dx, dy) in (0..16).flat_map(|dy| (0..16).map(move |dx| (dx, dy))) {
                    let stood_on = dx < width && dy < height && cells.binary_search(&cell(x + dx, y + dy)).is_ok();
                    assert_eq!(occupied_rows[dy as usize] >> dx & 1 == 1, stood_on, "tick {tick}: ({dx}, {dy}) of the {width}x{height} cells from ({x}, {y})");
                }
            }
        }
    }
    assert!(stayed_count > 1000 && refused_count > 10 && crossed_count > 100, "{stayed_count} stayed, {refused_count} refused, {crossed_count} crossed");
    assert!(entity_store.len() > 900, "they bred");
}
