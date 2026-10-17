//! Sheep on grass, on a plain the server makes: they eat it, starve
//! without it, breed lambs that grow up -- attributes coming and going
//! -- walk to the nearest grass when hungry, however far off within
//! their reach, leave thin pasture, never stand two on a cell, never
//! walk off the hot bitplanes, and tick the same on any number of
//! threads.

use crate::worlds::{cells_of_grass, first_superchunk, plain_start, plant_grass, put_entity, tick_sheep};
use coordinates::CellCartesian;
use entity_manager::{set_attribute, AttributeBlock, EntityRef, Header, WholeAttribute};
use entity_rules::sheep::{BIRTHS_COUNT, DEATHS_COUNT, EATEN_COUNT, FAR_COUNT, PATHS, SOUGHT_COUNT, WOKEN_COUNT, GESTATION_TIME, TIME_BETWEEN_MEALS, ROAMING_TIME, SHEEP, SHEEP_STATE, TIME_TO_STARVE, SheepState, STEP_JITTER, STEP};

/// The state of a sheep next hungry at `hungry_at`, in the middle of nothing.
fn hungry_at(hungry_at: u64) -> WholeAttribute {
    WholeAttribute::holding_value(SHEEP_STATE, SheepState { hungry_at, ..SheepState::default() })
}

/// What `sheep_entity` keeps: every sheep has its block.
fn sheep_state(sheep_entity: &EntityRef) -> SheepState {
    sheep_entity.attribute(SHEEP_STATE).expect("a sheep's state, always")
}

/// Makes about one sheep in `one_in` of `world` pregnant, its lamb
/// due at a tick within `due_within` of now and the sheep awake for it:
/// which sheep and when, drawn from the run's seed. A gestation is
/// longer than the tests that want a birth among much else run for.
fn with_lambs_due(world: &mut server::World, seed: u64, one_in: u64, due_within: u64) {
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    let tick_now = world.entity_store().current_tick();
    let every_sheep: Vec<(Header, SheepState, Vec<AttributeBlock>)> = world.entity_store().iter().map(|sheep_entity| (sheep_entity.header, sheep_state(&sheep_entity), sheep_entity.attributes.to_vec())).collect();
    for (header, state_was, mut attributes) in every_sheep {
        let due_tick = tick_now + 1 + seeded_random.draw_below(due_within);
        if seeded_random.draw_below(one_in) != 0 {
            continue;
        }
        set_attribute(&mut attributes, SHEEP_STATE, SheepState { lamb_due_at: due_tick, ..state_was });
        assert_eq!(world.remove_entity(&header).entities_removed, 1);
        assert_eq!(world.put_entity(Header { wake: header.wake.min(due_tick), ..header }, &attributes).puts, 1);
    }
}

/// Every sheep knows when it is next hungry, is a sheep, and is
/// never both a lamb and pregnant.
fn well_formed(sheep_entity: EntityRef) {
    assert_eq!(sheep_entity.header.entity_type, SHEEP);
    let checked_state = sheep_state(&sheep_entity);
    assert!(checked_state.grown_at == 0 || checked_state.lamb_due_at == 0, "a lamb, pregnant");
}

/// With no grass at all, every sheep starves, within the wakes it
/// starves at.
#[test]
fn sheep_without_grass_starve() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(1, 0, 500, 1) });
    let (mut eaten_total, mut deaths_total) = (0, 0);
    for _ in 0..TIME_BETWEEN_MEALS.ticks() + TIME_TO_STARVE.ticks() + 2 * (STEP.ticks() + STEP_JITTER.ticks()) {
        let tick_counts = tick_sheep(&mut world).rules;
        (eaten_total, deaths_total) = (eaten_total + tick_counts[EATEN_COUNT], deaths_total + tick_counts[DEATHS_COUNT]);
    }
    assert_eq!((eaten_total, deaths_total, world.entity_store().len()), (0, 500, 0));
}

/// On grass, sheep eat -- each cell eaten turned to dirt -- breed, and
/// their lambs grow up: pregnancy and youth added and removed as they
/// go, and no sheep walks off the hot superchunks.
#[test]
fn sheep_eat_breed_and_grow_up() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 18_750, 400, 2) });
    let (mut eaten_total, mut births_total, mut lost, mut lambs_seen, mut pregnant_seen) = (0, 0, 0, false, false);
    // A gestation and six meals' time: the lambs conceived over those meals born, whatever either lasts.
    for tick in 0..GESTATION_TIME.ticks() + 6 * TIME_BETWEEN_MEALS.ticks() {
        let report = tick_sheep(&mut world);
        (eaten_total, births_total, lost) = (eaten_total + report.rules[EATEN_COUNT], births_total + report.rules[BIRTHS_COUNT], lost + report.instructions_applied.lost);
        if tick % 500 == 0 {
            for sheep_entity in world.entity_store().iter() {
                well_formed(sheep_entity);
                lambs_seen |= sheep_state(&sheep_entity).grown_at != 0;
                pregnant_seen |= sheep_state(&sheep_entity).lamb_due_at != 0;
            }
        }
    }
    assert!(eaten_total > 5_000 && births_total > 50, "{eaten_total} eaten, {births_total} born");
    assert!(lambs_seen && pregnant_seen);
    assert_eq!(lost, 0, "no sheep walks off the hot superchunks");
    assert!(world.entity_store().iter().any(|grown_entity| sheep_state(&grown_entity).grown_at == 0), "grown sheep");
}

/// A hungry sheep with no grass beside it walks the shortest way to the
/// nearest in the area about it: one eight cells from the only grass
/// stands on it eight wakes on, eats it, and no path was looked for
/// that was not found.
#[test]
fn hungry_sheep_walk_to_the_nearest_grass() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(1, 0, 0, 1) });
    let superchunk = first_superchunk(&world);
    let corner_cartesian = superchunk.top_left().cartesian();
    let (sheep_cartesian, grass_cartesian) = (CellCartesian { x: corner_cartesian.x + 500, y: corner_cartesian.y + 500 }, CellCartesian { x: corner_cartesian.x + 506, y: corner_cartesian.y + 493 });
    plant_grass(&mut world, grass_cartesian, 1, 1);
    let header = Header { entity_type: SHEEP, stood_cell: sheep_cartesian.into(), wake: 0 };
    put_entity(&mut world, header, &hungry_at(0));
    let (mut counts_so_far, mut ate_at) = (actions::RuleCounts::default(), None);
    for _ in 0..12 * (STEP.ticks() + STEP_JITTER.ticks()) {
        // The grass rule left out: the one cell of grass must stay until eaten.
        let report = tick_sheep(&mut world);
        if report.rules[EATEN_COUNT] > 0 && ate_at.is_none() {
            ate_at = Some(counts_so_far[WOKEN_COUNT]);
        }
        counts_so_far += report.rules;
    }
    assert_eq!(counts_so_far[EATEN_COUNT], 1, "the one cell of grass, eaten");
    assert_eq!(ate_at, Some(7), "seven steps to it, eaten on the wake after");
    assert_eq!((counts_so_far[SOUGHT_COUNT], counts_so_far[PATHS]), (6, 6), "a path found each step until the grass was beside it");
    assert_eq!(cells_of_grass(&world), 0);
}

/// A hungry sheep with no grass in the area about it looks further
/// off, to its reach: one 150 cells across and 100 down from the only
/// grass, in the next superchunk, walks straight to it -- 150 steps,
/// most of what it can take before it starves -- and eats it.
#[test]
fn hungry_sheep_walk_to_grass_far_off() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 0, 0, 2) });
    // The square's top left superchunk: the first, row by row.
    let corner_cartesian = first_superchunk(&world).top_left().cartesian();
    let (sheep_cartesian, grass_cartesian) = (CellCartesian { x: corner_cartesian.x + 900, y: corner_cartesian.y + 700 }, CellCartesian { x: corner_cartesian.x + 1050, y: corner_cartesian.y + 800 });
    plant_grass(&mut world, grass_cartesian, 1, 1);
    let header = Header { entity_type: SHEEP, stood_cell: sheep_cartesian.into(), wake: 0 };
    put_entity(&mut world, header, &hungry_at(0));
    let (mut counts_so_far, mut ate_at) = (actions::RuleCounts::default(), None);
    for _ in 0..TIME_TO_STARVE.ticks() {
        let report = tick_sheep(&mut world);
        if report.rules[EATEN_COUNT] > 0 && ate_at.is_none() {
            ate_at = Some(counts_so_far[WOKEN_COUNT]);
        }
        counts_so_far += report.rules;
        // No further: fed, it is hungry again within the time it would have starved in, and seeks grass there is none of.
        if ate_at.is_some() {
            break;
        }
    }
    assert_eq!(counts_so_far[EATEN_COUNT], 1, "the one cell of grass, eaten");
    assert_eq!(ate_at, Some(150), "as many steps as the further of across and down, eaten on the wake after");
    assert_eq!(counts_so_far[SOUGHT_COUNT], counts_so_far[PATHS], "a way found every step");
    assert!(counts_so_far[FAR_COUNT] > 135 && counts_so_far[FAR_COUNT] < 150, "{} of them from far off", counts_so_far[FAR_COUNT]);
}

/// A sheep that eats on thin pasture leaves it: hungry again, it walks
/// one way for [`ROAM`], a step a wake, before it looks for
/// grass. Sixteen of them, a superchunk and its random numbers each:
/// one may die of old age on the way, on any seed, and those that
/// live are judged.
#[test]
fn sheep_on_thin_pasture_roam_away() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(16, 0, 0, 1) });
    let start_cells: Vec<CellCartesian> = world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| superchunk.top_left().cartesian()).map(|corner_cartesian| CellCartesian { x: corner_cartesian.x + 500, y: corner_cartesian.y + 500 }).collect();
    for &start_cell in &start_cells {
        plant_grass(&mut world, start_cell, 1, 1);
        put_entity(&mut world, Header { entity_type: SHEEP, stood_cell: start_cell.into(), wake: 0 }, &hungry_at(0));
    }
    // A sheep is told from the others by its superchunk, which it does not walk out of in so few steps.
    let superchunks = world.bitmap_arena().superchunk_indices();
    let index_of_sheep = |sheep_asked: &EntityRef| superchunks.binary_search(&sheep_asked.header.stood_cell.superchunk()).expect("on a hot superchunk");
    // Each sheep: whether it has set off, and where its steps ran out.
    let (mut eaten_total, mut sheep_walks) = (0, vec![(false, None); start_cells.len()]);
    for _ in 0..TIME_BETWEEN_MEALS.ticks() + ROAMING_TIME.ticks() + 4 * (STEP.ticks() + STEP_JITTER.ticks()) {
        eaten_total += tick_sheep(&mut world).rules[EATEN_COUNT];
        for sheep_entity in world.entity_store().iter() {
            let (is_roaming, (set_off, came_to)) = (sheep_state(&sheep_entity).roaming_until != 0, &mut sheep_walks[index_of_sheep(&sheep_entity)]);
            if *set_off && !is_roaming && came_to.is_none() {
                *came_to = Some(sheep_entity.header.stood_cell.cartesian());
            }
            *set_off |= is_roaming;
        }
    }
    assert_eq!(eaten_total, start_cells.len() as u64, "each its one cell of grass, eaten: thin pasture");
    let alive_sheep: Vec<usize> = world.entity_store().iter().map(|alive_entity| index_of_sheep(&alive_entity)).collect();
    assert!(!alive_sheep.is_empty(), "sixteen sheep, every one dead of old age in so few ticks");
    for sheep_index in alive_sheep {
        let (walk_start, came_to) = (start_cells[sheep_index], sheep_walks[sheep_index].1.expect("it set off, and its steps ran out"));
        let cells_apart = (came_to.x.abs_diff(walk_start.x)).max(came_to.y.abs_diff(walk_start.y));
        let steps = ROAMING_TIME.ticks() / (STEP.ticks() + STEP_JITTER.ticks())..=ROAMING_TIME.ticks() / STEP.ticks() + 1;
        assert!(steps.contains(&(cells_apart as u64)), "{cells_apart} cells off: one way, every step");
    }
}

/// Grass and sheep over four superchunks, across their borders, come out
/// the same on one thread and on four.
#[test]
fn any_number_of_threads_ticks_sheep_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let run_with_threads = |thread_count| {
        let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 12_500, 300, thread_count) });
        let reports: Vec<_> = (0..1500).map(|_| tick_sheep(&mut world)).map(|report| (report.rules, report.instructions_applied)).collect();
        let sheep_list: Vec<_> = world.entity_store().iter().map(|sheep_entity| (sheep_entity.header, sheep_entity.attributes.to_vec())).collect();
        (reports, sheep_list, cells_of_grass(&world))
    };
    let (on_one_thread, on_four_threads) = (run_with_threads(1), run_with_threads(4));
    assert!(!on_one_thread.1.is_empty());
    assert_eq!(on_one_thread, on_four_threads);
}

/// Sheep never overlap: a crowded flock, eating, breeding and walking
/// over four superchunks and their borders, stands one to a cell after
/// every tick checked.
#[test]
fn sheep_never_overlap() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut world = server::start_world(server::WorldStart { seed, ..plain_start(4, 18_750, 60_000, 4) });
    assert_eq!(world.entity_store().len(), 240_000, "each on a cell of its own from the start");
    with_lambs_due(&mut world, seed, 16, 2_000);
    let (mut moves_total, mut births_total) = (0, 0);
    for tick in 0..2_000 {
        let report = tick_sheep(&mut world);
        (moves_total, births_total) = (moves_total + report.instructions_applied.moves, births_total + report.rules[BIRTHS_COUNT]);
        if tick % 100 == 99 {
            let mut cells: Vec<_> = world.entity_store().iter().map(|sheep_entity| sheep_entity.header.stood_cell).collect();
            cells.sort_unstable();
            assert!(cells.windows(2).all(|adjacent_pair| adjacent_pair[0] != adjacent_pair[1]), "tick {tick}: two sheep on a cell");
        }
    }
    // They see where the others stand, in the collision plane, and seldom step for one cell in one tick: stepping is what is asked for.
    assert!(moves_total > 10_000, "{moves_total} moves");
    assert!(births_total > 0);
}

/// A sheep's meal lost to a decay, made to happen: a hungry sheep on a
/// cell of grass, the sheep's rule run on a tick that first clears
/// that very cell as grass decaying does. The cell is cleared once;
/// every write of the meal is refused -- nothing eaten is counted, the sheep is
/// as hungry as it was and loses nothing it had -- and it sleeps a
/// step and wakes to look again. On the cell beside it, a sheep whose
/// grass stands eats as ever.
#[test]
fn a_meal_lost_to_a_decay_leaves_the_sheep_hungry() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
    use coordinates::CellIndex;
    use entity_manager::WorldEntities;
    use actions::cells;
    use type_registry::{GRASS, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};
    let superchunk = coordinates::WORLD_MIDDLE;
    let (mut layer_codec, mut bitmap_arena, mut entity_store) = (chunk_storage::LayerCodec::new(), BitmapArena::new(), WorldEntities::new());
    for chunk in superchunk.chunks() {
        for layer_type in [GRASS, WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST] {
            bitmap_arena.make_hot(BucketKey { layer_type, chunk }, None, &mut layer_codec);
        }
    }
    entity_store.keep_superchunks(&bitmap_arena.superchunk_indices());
    let corner_cartesian = superchunk.top_left().cartesian();
    let (decaying_cell, standing_cell): (CellIndex, CellIndex) = (CellCartesian { x: corner_cartesian.x + 400, y: corner_cartesian.y + 400 }.into(), CellCartesian { x: corner_cartesian.x + 600, y: corner_cartesian.y + 400 }.into());
    for grass_cell in [decaying_cell, standing_cell] {
        bitmap_arena.queue_write(GRASS, Write::cell(grass_cell, WriteOp::SetCell));
    }
    bitmap_arena.apply();
    let was_hungry_at = 0;
    entity_store.queue_put(Header { entity_type: SHEEP, stood_cell: decaying_cell, wake: 0 }, &hungry_at(was_hungry_at));
    entity_store.queue_put(Header { entity_type: SHEEP, stood_cell: standing_cell, wake: 0 }, &hungry_at(was_hungry_at));
    entity_store.apply();
    let mut test_simulation = simulation::Simulation::new(1);
    let report = test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn: &mut actions::Turn| {
        // The decay first, counted under 0; the sheep's counts from 8 on.
        cells::clear_counted(turn, GRASS, decaying_cell, 0);
        turn.count_under(8);
        entity_rules::sheep::rule(turn)
    });
    assert_eq!((bitmap_arena.layer_holds_cell(GRASS, decaying_cell), bitmap_arena.layer_holds_cell(GRASS, standing_cell)), (Ok(false), Ok(false)));
    assert_eq!((report.writes_applied.cells_changed, report.writes_applied.writes_refused), (2, 1), "one meal applied, one refused");
    // What each comes to is a write an attribute changed and a move, all of the one applied and of the other refused; and of the two sleeps, the other way about.
    let (compares_applied, compares_refused) = report.instructions_compared;
    assert!(compares_applied >= 2 && compares_refused >= 2, "{compares_applied} applied, {compares_refused} refused");
    assert_eq!((report.counted_when_applied[0], report.counted_when_applied[8 + EATEN_COUNT], report.rules[EATEN_COUNT]), (1, 1, 0), "a decay and one meal, counted as applied");
    let sheep_list: Vec<_> = entity_store.iter().collect();
    // One may die of old age before so long a sleep as a meal's, on any seed: its death is its meal's, both or neither.
    let fed_sheep = sheep_list.iter().find(|sheep_checked| sheep_checked.header.stood_cell == standing_cell);
    assert_eq!(fed_sheep.is_none() as u64, report.counted_when_applied[8 + entity_rules::sheep::DEATHS_COUNT]);
    assert!(fed_sheep.is_none_or(|fed_entity| sheep_state(fed_entity).hungry_at == TIME_BETWEEN_MEALS.ticks() && fed_entity.header.wake >= TIME_BETWEEN_MEALS.ticks()), "fed, and asleep until hungry");
    let hungry_sheep = sheep_list.iter().find(|sheep_found| sheep_found.header.stood_cell == decaying_cell).expect("the sheep whose meal was refused, alive");
    assert_eq!((sheep_state(hungry_sheep), hungry_sheep.header.stood_cell), (SheepState { hungry_at: was_hungry_at, ..SheepState::default() }, decaying_cell), "as it was");
    assert!((STEP.ticks()..STEP.ticks() + STEP_JITTER.ticks()).contains(&hungry_sheep.header.wake), "asleep a step: it wakes at {}", hungry_sheep.header.wake);
}
