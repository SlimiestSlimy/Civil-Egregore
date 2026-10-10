//! Sheep on grass, on a plain the server makes: they eat it, starve
//! without it, breed lambs that grow up -- attributes coming and going
//! -- walk to the nearest grass when hungry, however far off within
//! their reach, leave thin pasture, never stand two on a cell, never
//! walk off the hot bitplanes, and tick the same on any number of
//! threads.

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, put_entity, tick_sheep};
use coordinates::CellCartesian;
use entity_manager::{AttributeBlock, EntityId, EntityRef, Header};
use entity_rules::sheep::{BIRTHS, DEATHS, EATEN, FAR, PATHS, SOUGHT, WOKEN, HUNGRY_AT, LAMB, MEAL_TICKS, PREGNANT, ROAMING, ROAM_TICKS, SHEEP, STARVE_TICKS, STEP_JITTER, STEP_TICKS};

/// Every sheep knows when it is next hungry, is a sheep, and is
/// never both a lamb and pregnant.
fn well_formed(sheep: EntityRef) {
    assert_eq!(sheep.header.kind, SHEEP);
    assert!(sheep.attribute(HUNGRY_AT).is_some(), "when it is next hungry, always");
    assert!(sheep.attribute(LAMB).is_none() || sheep.attribute(PREGNANT).is_none(), "a lamb, pregnant");
}

/// With no grass at all, every sheep starves, within the wakes it
/// starves at.
#[test]
fn sheep_without_grass_starve() {
    let mut world = plain_world(1, 0, 500, 1);
    let (mut eaten, mut deaths) = (0, 0);
    for _ in 0..MEAL_TICKS + STARVE_TICKS + 2 * (STEP_TICKS + STEP_JITTER) {
        let done = tick_sheep(&mut world).rules;
        (eaten, deaths) = (eaten + done[EATEN], deaths + done[DEATHS]);
    }
    assert_eq!((eaten, deaths, world.entities().len()), (0, 500, 0));
}

/// On grass, sheep eat -- each cell eaten turned to dirt -- breed, and
/// their lambs grow up: pregnancy and youth added and removed as they
/// go, and no sheep walks off the hot superchunks.
#[test]
fn sheep_eat_breed_and_grow_up() {
    let mut world = plain_world(4, 300_000, 400, 2);
    let (mut eaten, mut births, mut lost, mut lambs_seen, mut pregnant_seen) = (0, 0, 0, false, false);
    for tick in 0..40_000 {
        let report = tick_sheep(&mut world);
        (eaten, births, lost) = (eaten + report.rules[EATEN], births + report.rules[BIRTHS], lost + report.instructions_applied.lost);
        if tick % 500 == 0 {
            for sheep in world.entities().iter() {
                well_formed(sheep);
                lambs_seen |= sheep.attribute(LAMB).is_some();
                pregnant_seen |= sheep.attribute(PREGNANT).is_some();
            }
        }
    }
    assert!(eaten > 5_000 && births > 100, "{eaten} eaten, {births} born");
    assert!(lambs_seen && pregnant_seen);
    assert_eq!(lost, 0, "no sheep walks off the hot superchunks");
    assert!(world.entities().iter().any(|sheep| sheep.attribute(LAMB).is_none() && sheep.attribute(HUNGRY_AT).is_some()), "grown sheep");
}

/// A hungry sheep with no grass beside it walks the shortest way to the
/// nearest in the area about it: one eight cells from the only grass
/// stands on it eight wakes on, eats it, and no path was looked for
/// that was not found.
#[test]
fn hungry_sheep_walk_to_the_nearest_grass() {
    let mut world = plain_world(1, 0, 0, 1);
    let superchunk = first_superchunk(&world);
    let corner = superchunk.top_left().cartesian();
    let (sheep, grass) = (CellCartesian { x: corner.x + 500, y: corner.y + 500 }, CellCartesian { x: corner.x + 506, y: corner.y + 493 });
    plant_grass(&mut world, grass, 1, 1);
    let header = Header { id: EntityId(1), kind: SHEEP, at: sheep.into(), wake: 0 };
    put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    let (mut done, mut ate_at) = (instructions::RuleCounts::default(), None);
    for _ in 0..12 * (STEP_TICKS + STEP_JITTER) {
        // The grass rule left out: the one cell of grass must stay until eaten.
        let report = tick_sheep(&mut world);
        if report.rules[EATEN] > 0 && ate_at.is_none() {
            ate_at = Some(done[WOKEN]);
        }
        done += report.rules;
    }
    assert_eq!(done[EATEN], 1, "the one cell of grass, eaten");
    assert_eq!(ate_at, Some(7), "seven steps to it, eaten on the wake after");
    assert_eq!((done[SOUGHT], done[PATHS]), (6, 6), "a path found each step until the grass was beside it");
    assert_eq!(cells_of_grass(&world), 0);
}

/// A hungry sheep with no grass in the area about it looks further
/// off, to its reach: one 150 cells across and 100 down from the only
/// grass, in the next superchunk, walks straight to it -- 150 steps,
/// most of what it can take before it starves -- and eats it.
#[test]
fn hungry_sheep_walk_to_grass_far_off() {
    let mut world = plain_world(4, 0, 0, 2);
    // The square's top left superchunk: the first, row by row.
    let corner = first_superchunk(&world).top_left().cartesian();
    let (sheep, grass) = (CellCartesian { x: corner.x + 900, y: corner.y + 700 }, CellCartesian { x: corner.x + 1050, y: corner.y + 800 });
    plant_grass(&mut world, grass, 1, 1);
    let header = Header { id: EntityId(1), kind: SHEEP, at: sheep.into(), wake: 0 };
    put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    let (mut done, mut ate_at) = (instructions::RuleCounts::default(), None);
    for _ in 0..STARVE_TICKS {
        let report = tick_sheep(&mut world);
        if report.rules[EATEN] > 0 && ate_at.is_none() {
            ate_at = Some(done[WOKEN]);
        }
        done += report.rules;
    }
    assert_eq!(done[EATEN], 1, "the one cell of grass, eaten");
    assert_eq!(ate_at, Some(150), "as many steps as the further of across and down, eaten on the wake after");
    assert_eq!(done[SOUGHT], done[PATHS], "a way found every step");
    assert!(done[FAR] > 135 && done[FAR] < 150, "{} of them from far off", done[FAR]);
}

/// A sheep that eats on thin pasture leaves it: hungry again, it walks
/// one way for [`ROAM_TICKS`], a step a wake, before it looks for
/// grass. Sixteen of them, a superchunk and its random numbers each:
/// one may die of old age on the way, on any seed, and those that
/// live are judged.
#[test]
fn sheep_on_thin_pasture_roam_away() {
    let mut world = plain_world(16, 0, 0, 1);
    let starts: Vec<CellCartesian> = world.arena().superchunk_indices().into_iter().map(|superchunk| superchunk.top_left().cartesian()).map(|corner| CellCartesian { x: corner.x + 500, y: corner.y + 500 }).collect();
    for (id, &start) in starts.iter().enumerate() {
        plant_grass(&mut world, start, 1, 1);
        put_entity(&mut world, Header { id: EntityId(id as u64), kind: SHEEP, at: start.into(), wake: 0 }, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    }
    // Each sheep: whether it has set off, and where its steps ran out.
    let (mut eaten, mut walks) = (0, vec![(false, None); starts.len()]);
    for _ in 0..MEAL_TICKS + ROAM_TICKS + 4 * (STEP_TICKS + STEP_JITTER) {
        eaten += tick_sheep(&mut world).rules[EATEN];
        for sheep in world.entities().iter() {
            let (roaming, (set_off, came_to)) = (sheep.attribute(ROAMING).is_some(), &mut walks[sheep.header.id.0 as usize]);
            if *set_off && !roaming && came_to.is_none() {
                *came_to = Some(sheep.header.at.cartesian());
            }
            *set_off |= roaming;
        }
    }
    assert_eq!(eaten, starts.len() as u64, "each its one cell of grass, eaten: thin pasture");
    let alive: Vec<usize> = world.entities().iter().map(|sheep| sheep.header.id.0 as usize).collect();
    assert!(!alive.is_empty(), "sixteen sheep, every one dead of old age in so few ticks");
    for sheep in alive {
        let (start, came_to) = (starts[sheep], walks[sheep].1.expect("it set off, and its steps ran out"));
        let apart = (came_to.x.abs_diff(start.x)).max(came_to.y.abs_diff(start.y));
        let steps = ROAM_TICKS / (STEP_TICKS + STEP_JITTER)..=ROAM_TICKS / STEP_TICKS + 1;
        assert!(steps.contains(&(apart as u64)), "{apart} cells off: one way, every step");
    }
}

/// Grass and sheep over four superchunks, across their borders, come out
/// the same on one thread and on four.
#[test]
fn any_number_of_threads_ticks_sheep_the_same() {
    let run = |threads| {
        let mut world = plain_world(4, 200_000, 300, threads);
        let reports: Vec<_> = (0..1500).map(|_| tick_sheep(&mut world)).map(|report| (report.rules, report.instructions_applied)).collect();
        let sheep: Vec<_> = world.entities().iter().map(|sheep| (sheep.header, sheep.attributes.to_vec())).collect();
        (reports, sheep, cells_of_grass(&world))
    };
    let (one, four) = (run(1), run(4));
    assert!(!one.1.is_empty());
    assert_eq!(one, four);
}

/// Sheep never overlap: a crowded flock, eating, breeding and walking
/// over four superchunks and their borders, stands one to a cell after
/// every tick checked.
#[test]
fn sheep_never_overlap() {
    let mut world = plain_world(4, 300_000, 60_000, 4);
    assert_eq!(world.entities().len(), 240_000, "each on a cell of its own from the start");
    let (mut stayed, mut births) = (0, 0);
    for tick in 0..2_000 {
        let report = tick_sheep(&mut world);
        (stayed, births) = (stayed + report.instructions_applied.stayed, births + report.rules[BIRTHS]);
        if tick % 100 == 99 {
            let mut cells: Vec<_> = world.entities().iter().map(|sheep| sheep.header.at).collect();
            cells.sort_unstable();
            assert!(cells.windows(2).all(|pair| pair[0] != pair[1]), "tick {tick}: two sheep on a cell");
        }
    }
    assert!(stayed > 1_000, "{stayed} sheep found their cell taken, and stayed");
    assert!(births > 0);
}

/// A sheep's meal lost to a decay, made to happen: a hungry sheep on a
/// cell of grass, the sheep's rule run on a tick that first clears
/// that very cell as grass decaying does. The cell is cleared once;
/// the meal is refused whole -- nothing eaten is counted, the sheep is
/// as hungry as it was and loses nothing it had -- and it sleeps a
/// step and wakes to look again. On the cell beside it, a sheep whose
/// grass stands eats as ever.
#[test]
fn a_meal_lost_to_a_decay_leaves_the_sheep_hungry() {
    use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
    use coordinates::CellIndex;
    use entity_manager::Entities;
    use instructions::cells;
    use type_registry::{GRASS, WALL_EAST, WALL_SOUTH};
    let superchunk = coordinates::WORLD_MIDDLE;
    let (mut codec, mut arena, mut entities) = (chunk_storage::LayerCodec::new(), BitmapArena::new(), Entities::new());
    for chunk in superchunk.chunks() {
        for layer_type in [GRASS, WALL_EAST, WALL_SOUTH] {
            arena.make_hot(BucketKey { layer_type, chunk }, None, &mut codec);
        }
    }
    entities.align(&arena.superchunk_indices());
    let corner = superchunk.top_left().cartesian();
    let (decaying, standing): (CellIndex, CellIndex) = (CellCartesian { x: corner.x + 400, y: corner.y + 400 }.into(), CellCartesian { x: corner.x + 600, y: corner.y + 400 }.into());
    for at in [decaying, standing] {
        arena.queue(GRASS, Write::cell(at, WriteOp::Set));
    }
    arena.apply();
    let was_hungry_at = 0;
    entities.queue_put(Header { id: EntityId(1), kind: SHEEP, at: decaying, wake: 0 }, &[AttributeBlock::holding(HUNGRY_AT, was_hungry_at)]);
    entities.queue_put(Header { id: EntityId(2), kind: SHEEP, at: standing, wake: 0 }, &[AttributeBlock::holding(HUNGRY_AT, was_hungry_at)]);
    entities.apply();
    let mut simulation = simulation::Simulation::new(1);
    let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut instructions::Turn, _: &mut Vec<CellIndex>| {
        // The decay first, counted under 0; the sheep's counts from 8 on.
        cells::clear_counted(turn, GRASS, decaying, 0);
        turn.count_under(8);
        entity_rules::sheep::rule(turn)
    });
    assert_eq!((arena.holds(GRASS, decaying), arena.holds(GRASS, standing)), (Ok(false), Ok(false)));
    assert_eq!((report.writes_applied.changed, report.writes_applied.refused, report.groups), (2, 1, (1, 1)), "one meal applied, one refused");
    assert_eq!((report.counted_when_applied[0], report.counted_when_applied[8 + EATEN], report.rules[EATEN]), (1, 1, 0), "a decay and one meal, counted as applied");
    let sheep: Vec<_> = entities.iter().collect();
    // One may die of old age before so long a sleep as a meal's, on any seed: its death is its meal's, both or neither.
    let fed = sheep.iter().find(|sheep| sheep.header.id == EntityId(2));
    assert_eq!(fed.is_none() as u64, report.counted_when_applied[8 + entity_rules::sheep::DEATHS]);
    assert!(fed.is_none_or(|fed| fed.attribute(HUNGRY_AT) == Some(MEAL_TICKS) && fed.header.wake >= MEAL_TICKS), "fed, and asleep until hungry");
    let hungry = sheep.iter().find(|sheep| sheep.header.id == EntityId(1)).expect("the sheep whose meal was refused, alive");
    assert_eq!((hungry.attribute(HUNGRY_AT), hungry.attribute(ROAMING), hungry.header.at), (Some(was_hungry_at), None, decaying), "as it was");
    assert!((STEP_TICKS..STEP_TICKS + STEP_JITTER).contains(&hungry.header.wake), "asleep a step: it wakes at {}", hungry.header.wake);
}
