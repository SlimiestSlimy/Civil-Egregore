//! Sheep on grass, on a plain the server makes: they eat it, starve
//! without it, breed lambs that grow up -- attributes coming and going
//! -- walk to the nearest grass when hungry, however far off within
//! their reach, leave thin pasture, never stand two on a cell, never
//! walk off the hot bitplanes, and tick the same on any number of
//! threads.

use protocol::server_parameters::{ParameterValue, SEED};
use crate::probe::{entities_of_the_turn, of_the_turn, Probe};
use crate::worlds::{base, cells_of_grass, every_entity, first_superchunk, hot_superchunks, nothing_more, plain_start, probed_world, put_entities, set_rectangle, tick_sheep};
use utilities::coordinates::{CellCartesian, CellIndex};
use server::simulation::simulation_api::{cell_reads_and_writes, entity_reads_and_writes, tick_and_random_stream, AttributeBlock, EntityRef, Header, RuleCounts, WholeAttribute};
use server::World;
use basegame::base_types::SHEEP_RULE;
use basegame::entity_rules::sheep::{BIRTHS_COUNT, DEATHS_COUNT, EATEN_COUNT, FAR_COUNT, PATHS, SOUGHT_COUNT, WOKEN_COUNT, GESTATION_TIME, TIME_BETWEEN_MEALS, ROAMING_TIME, TIME_TO_STARVE, SheepState, STEP_JITTER, STEP};

/// The attributes of a sheep next hungry at `hungry_at`, in the middle of nothing.
fn hungry_at(hungry_at: u64) -> Vec<AttributeBlock> {
    WholeAttribute::holding_value(base().sheep_state, SheepState { hungry_at, ..SheepState::default() }).to_vec()
}

/// What `sheep_entity` keeps: every sheep has its block.
fn sheep_state(sheep_entity: &EntityRef) -> SheepState {
    sheep_entity.attribute(base().sheep_state).expect("a sheep's state, always")
}

/// A hungry sheep on `sheep_cartesian` and the one cell of grass, on
/// `grass_cartesian`, set up in ticks of the probe alone.
fn a_sheep_and_its_grass(world: &mut World, probe: &Probe, sheep_cartesian: CellCartesian, grass_cartesian: CellCartesian) {
    set_rectangle(world, probe, base().grass, grass_cartesian, 1, 1);
    put_entities(world, probe, vec![(Header { entity_type: base().sheep, stood_cell: sheep_cartesian.into(), wake: 0 }, hungry_at(0))]);
}

/// Whether `sheep_entity` is well formed: a sheep, knowing when it is
/// next hungry, never both a lamb and pregnant. Read in a tick and
/// judged after it: a test's assert fails outside the tick's threads.
fn well_formed(sheep_entity: &EntityRef) -> bool {
    let checked_state = sheep_state(sheep_entity);
    sheep_entity.header.entity_type == base().sheep && (checked_state.grown_at == 0 || checked_state.lamb_due_at == 0)
}

/// With no grass at all, every sheep starves, within the wakes it
/// starves at.
#[test]
fn sheep_without_grass_starve() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(1, 0, 500, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let (mut eaten_total, mut deaths_total) = (0, 0);
    for _ in 0..TIME_BETWEEN_MEALS.ticks() + TIME_TO_STARVE.ticks() + 2 * (STEP.ticks() + STEP_JITTER.ticks()) {
        let tick_counts = tick_sheep(&mut world).rules;
        (eaten_total, deaths_total) = (eaten_total + tick_counts[EATEN_COUNT], deaths_total + tick_counts[DEATHS_COUNT]);
    }
    assert_eq!((eaten_total, deaths_total, every_entity(&mut world, &probe, |_| ()).len()), (0, 500, 0));
}

/// On grass, sheep eat -- each cell eaten turned to dirt -- breed, and
/// their lambs grow up: pregnancy and youth added and removed as they
/// go, and no sheep walks off the hot superchunks.
#[test]
fn sheep_eat_breed_and_grow_up() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(4, 18_750, 400, 2).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let (mut eaten_total, mut births_total, mut lost, mut lambs_seen, mut pregnant_seen) = (0, 0, 0, false, false);
    // A gestation and six meals' time: the lambs conceived over those meals born, whatever either lasts.
    for tick in 0..GESTATION_TIME.ticks() + 6 * TIME_BETWEEN_MEALS.ticks() {
        // Every 500th tick, the sheep read as it found them.
        let report = if tick % 500 == 0 {
            let (report, seen_states) = probe.tick_rule(&mut world, SHEEP_RULE, |tick_state| entities_of_the_turn(tick_state).iter().map(|sheep_entity| (well_formed(sheep_entity), sheep_state(sheep_entity))).collect());
            assert!(seen_states.iter().all(|&(is_well_formed, _)| is_well_formed), "tick {tick}: a sheep not a sheep, or a lamb pregnant");
            let seen_states: Vec<SheepState> = seen_states.into_iter().map(|(_, seen_state)| seen_state).collect();
            lambs_seen |= seen_states.iter().any(|seen_state| seen_state.grown_at != 0);
            pregnant_seen |= seen_states.iter().any(|seen_state| seen_state.lamb_due_at != 0);
            report
        } else {
            tick_sheep(&mut world)
        };
        (eaten_total, births_total, lost) = (eaten_total + report.rules[EATEN_COUNT], births_total + report.rules[BIRTHS_COUNT], lost + report.instructions_applied.lost);
    }
    assert!(eaten_total > 5_000 && births_total > 50, "{eaten_total} eaten, {births_total} born");
    assert!(lambs_seen && pregnant_seen);
    assert_eq!(lost, 0, "no sheep walks off the hot superchunks");
    assert!(every_entity(&mut world, &probe, sheep_state).iter().any(|grown_state| grown_state.grown_at == 0), "grown sheep");
}

/// A hungry sheep with no grass beside it walks the shortest way to the
/// nearest in the area about it: one eight cells from the only grass
/// stands on it eight wakes on, eats it, and no path was looked for
/// that was not found.
#[test]
fn hungry_sheep_walk_to_the_nearest_grass() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let corner_cartesian = first_superchunk(&mut world, &probe).top_left().cartesian();
    a_sheep_and_its_grass(&mut world, &probe, CellCartesian { x: corner_cartesian.x + 500, y: corner_cartesian.y + 500 }, CellCartesian { x: corner_cartesian.x + 506, y: corner_cartesian.y + 493 });
    let (mut counts_so_far, mut ate_at) = (RuleCounts::default(), None);
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
    assert_eq!(cells_of_grass(&mut world, &probe), 0);
}

/// A hungry sheep with no grass in the area about it looks further
/// off, to its reach: one 150 cells across and 100 down from the only
/// grass, in the next superchunk, walks straight to it -- 150 steps,
/// most of what it can take before it starves -- and eats it.
#[test]
fn hungry_sheep_walk_to_grass_far_off() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(4, 0, 0, 2).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    // The square's top left superchunk: the first, row by row.
    let corner_cartesian = first_superchunk(&mut world, &probe).top_left().cartesian();
    a_sheep_and_its_grass(&mut world, &probe, CellCartesian { x: corner_cartesian.x + 900, y: corner_cartesian.y + 700 }, CellCartesian { x: corner_cartesian.x + 1050, y: corner_cartesian.y + 800 });
    let (mut counts_so_far, mut ate_at) = (RuleCounts::default(), None);
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
/// one way for [`ROAMING_TIME`], a step a wake, before it looks for
/// grass. Sixteen of them, a superchunk and its random numbers each:
/// one may die of old age on the way, on any seed, and those that
/// live are judged.
#[test]
fn sheep_on_thin_pasture_roam_away() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(16, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let superchunks = hot_superchunks(&mut world, &probe);
    let start_cells: Vec<CellCartesian> = superchunks.iter().map(|superchunk| superchunk.top_left().cartesian()).map(|corner_cartesian| CellCartesian { x: corner_cartesian.x + 500, y: corner_cartesian.y + 500 }).collect();
    for &start_cell in &start_cells {
        set_rectangle(&mut world, &probe, base().grass, start_cell, 1, 1);
    }
    put_entities(&mut world, &probe, start_cells.iter().map(|&start_cell| (Header { entity_type: base().sheep, stood_cell: start_cell.into(), wake: 0 }, hungry_at(0))).collect());
    // A sheep is told from the others by its superchunk, which it does not walk out of in so few steps.
    let index_of_sheep = |sheep_cell: CellIndex| superchunks.binary_search(&sheep_cell.superchunk()).expect("on a hot superchunk");
    // Each sheep: whether it has set off, and where its steps ran out.
    let (mut eaten_total, mut sheep_walks) = (0, vec![(false, None); start_cells.len()]);
    for _ in 0..TIME_BETWEEN_MEALS.ticks() + ROAMING_TIME.ticks() + 4 * (STEP.ticks() + STEP_JITTER.ticks()) {
        // Each sheep as the tick found it: where it stands, and whether it roams.
        let (report, seen_sheep) = probe.tick_rule(&mut world, SHEEP_RULE, |tick_state| entities_of_the_turn(tick_state).iter().map(|sheep_entity| (sheep_entity.header.stood_cell, sheep_state(sheep_entity).roaming_until != 0)).collect());
        eaten_total += report.rules[EATEN_COUNT];
        for (sheep_cell, is_roaming) in seen_sheep {
            let (set_off, came_to) = &mut sheep_walks[index_of_sheep(sheep_cell)];
            if *set_off && !is_roaming && came_to.is_none() {
                *came_to = Some(sheep_cell.cartesian());
            }
            *set_off |= is_roaming;
        }
    }
    assert_eq!(eaten_total, start_cells.len() as u64, "each its one cell of grass, eaten: thin pasture");
    let alive_sheep: Vec<usize> = every_entity(&mut world, &probe, |alive_entity| alive_entity.header.stood_cell).into_iter().map(index_of_sheep).collect();
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
        let (mut world, probe) = probed_world(&plain_start(4, 12_500, 300, thread_count).with(SEED, ParameterValue::Hex(seed)), nothing_more);
        let reports: Vec<_> = (0..1500).map(|_| tick_sheep(&mut world)).map(|report| (report.rules, report.instructions_applied)).collect();
        let sheep_list = every_entity(&mut world, &probe, |sheep_entity| (sheep_entity.header, sheep_entity.attributes.to_vec()));
        (reports, sheep_list, cells_of_grass(&mut world, &probe))
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
    let (mut world, probe) = probed_world(&plain_start(4, 18_750, 60_000, 4).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    world.tick();
    // About one sheep in sixteen made pregnant, its lamb due within 2,000 ticks and the sheep awake for it -- one not
    // woken this tick, so its own rule writes nothing to it: which and when drawn from the run's seed and its cell.
    let (lamb_report, flock_count) = probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| {
        let (tick_now, every_sheep) = (tick_and_random_stream::current_tick(tick_state), entities_of_the_turn(tick_state));
        for sheep_entity in every_sheep.iter().filter(|sheep_entity| sheep_entity.header.wake > tick_now) {
            let mut sheep_random = utilities::rng::RandomSource::for_stream(seed, sheep_entity.header.stood_cell.0);
            let due_tick = tick_now + 1 + sheep_random.draw_below(2_000);
            if sheep_random.draw_below(16) == 0 {
                let state_was = sheep_state(sheep_entity);
                entity_reads_and_writes::set_attribute_of(tick_state, &sheep_entity.header, base().sheep_state, Some(state_was), SheepState { lamb_due_at: due_tick, ..state_was });
                entity_reads_and_writes::sleep(tick_state, &sheep_entity.header, sheep_entity.header.wake.min(due_tick));
            }
        }
        vec![every_sheep.len()]
    });
    assert_eq!(flock_count.iter().sum::<usize>(), 240_000, "each on a cell of its own from the first tick");
    let (mut moves_total, mut births_total) = (lamb_report.instructions_applied.moves, lamb_report.rules[BIRTHS_COUNT]);
    for tick in 0..2_000 {
        // Every 100th tick, where every sheep stands as the tick found it.
        let (report, mut cells) = probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| if tick % 100 == 99 { entities_of_the_turn(tick_state).iter().map(|sheep_entity| sheep_entity.header.stood_cell).collect() } else { Vec::new() });
        (moves_total, births_total) = (moves_total + report.instructions_applied.moves, births_total + report.rules[BIRTHS_COUNT]);
        cells.sort_unstable();
        assert!(cells.windows(2).all(|adjacent_pair| adjacent_pair[0] != adjacent_pair[1]), "tick {tick}: two sheep on a cell");
    }
    // They see where the others stand, in the collision plane, and seldom step for one cell in one tick: stepping is what is asked for.
    assert!(moves_total > 10_000, "{moves_total} moves");
    assert!(births_total > 0);
}

/// A sheep's meal lost to a decay, made to happen: a hungry sheep on a
/// cell of grass, the sheep's rule run on a tick whose probe first
/// clears that very cell as grass decaying does. The cell is cleared
/// once; every write of the meal is refused -- nothing eaten is
/// counted, the sheep is as hungry as it was and loses nothing it had
/// -- and it sleeps a step and wakes to look again. On a cell apart, a
/// sheep whose grass stands eats as ever.
#[test]
fn a_meal_lost_to_a_decay_leaves_the_sheep_hungry() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, probe) = probed_world(&plain_start(1, 0, 0, 1).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let corner_cartesian = first_superchunk(&mut world, &probe).top_left().cartesian();
    let (decaying_cell, standing_cell): (CellIndex, CellIndex) = (CellCartesian { x: corner_cartesian.x + 400, y: corner_cartesian.y + 400 }.into(), CellCartesian { x: corner_cartesian.x + 600, y: corner_cartesian.y + 400 }.into());
    for grass_cell in [decaying_cell, standing_cell] {
        set_rectangle(&mut world, &probe, base().grass, grass_cell.cartesian(), 1, 1);
    }
    let was_hungry_at = 0;
    put_entities(&mut world, &probe, [decaying_cell, standing_cell].map(|sheep_cell| (Header { entity_type: base().sheep, stood_cell: sheep_cell, wake: 0 }, hungry_at(was_hungry_at))).to_vec());
    // The decay, counted as the probe's first; the sheep's rule, woken that tick.
    let (report, meeting_tick) = probe.tick_rule(&mut world, SHEEP_RULE, move |tick_state| {
        if !of_the_turn(tick_state, decaying_cell) {
            return Vec::new();
        }
        cell_reads_and_writes::clear_counted(tick_state, base().grass, decaying_cell, 0);
        vec![tick_and_random_stream::current_tick(tick_state)]
    });
    let meeting_tick = meeting_tick[0];
    let grass_left = probe.alone(&mut world, move |tick_state| [decaying_cell, standing_cell].into_iter().filter(|&grass_cell| of_the_turn(tick_state, grass_cell)).map(|grass_cell| cell_reads_and_writes::layer_holds(tick_state, base().grass, grass_cell)).collect());
    assert_eq!(grass_left, [false, false]);
    assert_eq!((report.writes_applied.cells_changed, report.writes_applied.writes_refused), (2, 1), "one meal applied, one refused");
    // What each comes to is a write an attribute changed and a move, all of the one applied and of the other refused; and of the two sleeps, the other way about.
    let (compares_applied, compares_refused) = report.instructions_compared;
    assert!(compares_applied >= 2 && compares_refused >= 2, "{compares_applied} applied, {compares_refused} refused");
    assert_eq!((report.counted_when_applied[0], report.rules[EATEN_COUNT]), (1, 1), "a decay and one meal, counted as applied");
    let sheep_list = every_entity(&mut world, &probe, |sheep_entity| (sheep_entity.header, sheep_state(sheep_entity)));
    // One may die of old age before so long a sleep as a meal's, on any seed: its death is its meal's, both or neither.
    let fed_sheep = sheep_list.iter().find(|(header, _)| header.stood_cell == standing_cell);
    assert_eq!(fed_sheep.is_none() as u64, report.rules[DEATHS_COUNT]);
    assert!(fed_sheep.is_none_or(|(header, fed_state)| fed_state.hungry_at == meeting_tick + TIME_BETWEEN_MEALS.ticks() && header.wake >= meeting_tick + TIME_BETWEEN_MEALS.ticks()), "fed, and asleep until hungry");
    let (hungry_header, hungry_state) = sheep_list.iter().find(|(header, _)| header.stood_cell == decaying_cell).expect("the sheep whose meal was refused, alive");
    assert_eq!((*hungry_state, hungry_header.stood_cell), (SheepState { hungry_at: was_hungry_at, ..SheepState::default() }, decaying_cell), "as it was");
    assert!((meeting_tick + STEP.ticks()..meeting_tick + STEP.ticks() + STEP_JITTER.ticks()).contains(&hungry_header.wake), "asleep a step: it wakes at {}", hungry_header.wake);
}
