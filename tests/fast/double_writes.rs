//! Two writes on one cell in a tick: a tick's counts are the cells
//! that changed, to the cell. No two sheep eat one cell; a cell of
//! grass spread onto twice is one spread, one decayed and eaten is one
//! of the two; a tree put twice is one tree put; and of two lambs put
//! on one cell the one refused is born later
//! (`docs/server.md`, "Two writes on one cell").

use crate::worlds::{cells_of_grass, first_superchunk, plain_start, plant_grass, put_entity, tick_sheep};
use bitplane_manager::Write;
use coordinates::{CellCartesian, CellIndex};
use entity_manager::{Header, WholeAttribute, NEVER_WAKES};
use entity_rules::sheep::{SheepState, BIRTHS_COUNT, EATEN_COUNT, SHEEP, SHEEP_STATE, STEP_JITTER, STEP};
use sca_rules::{grass, trees};
use server::World;
use std::collections::HashSet;
use type_registry::{TREE, TREE_STAGE};
use utilities::rng::RandomSource;

/// The cell `(x, y)` cells from the top left of `world`.
fn cell(world: &World, x: u32, y: u32) -> CellIndex {
    let top_left_cartesian = first_superchunk(world).top_left().cartesian();
    CellCartesian { x: top_left_cartesian.x + x, y: top_left_cartesian.y + y }.into()
}

/// Of the cells that changed in a tick, with the grass `grass_grew` cells
/// more after it: those set and those cleared.
fn set_and_cleared(cells_changed: u64, grass_grew: i64) -> (u64, u64) {
    assert_eq!((cells_changed as i64 + grass_grew) % 2, 0, "{cells_changed} cells changed, {grass_grew} more grass");
    (((cells_changed as i64 + grass_grew) / 2) as u64, ((cells_changed as i64 - grass_grew) / 2) as u64)
}

/// No two sheep eat one cell: a crowd of hungry sheep on a square of
/// grass, the sheep's rule alone -- every tick, the cells eaten are as
/// many writes, and as many cells changed.
#[test]
fn no_two_sheep_eat_one_cell() {
    const CROWD_SIDE: u32 = 200;
    let mut world = server::start_world(server::WorldStart { seed: crate::test_seed::TestSeedHandle::open().seed(), ..plain_start(1, 0, 0, 2) });
    let mut test_random = RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    let crowd_corner_cartesian = cell(&world, 400, 400).cartesian();
    plant_grass(&mut world, crowd_corner_cartesian, CROWD_SIDE as u8, CROWD_SIDE as u8);
    let mut taken_offsets = HashSet::new();
    while taken_offsets.len() < 10_000 {
        let (x, y) = (test_random.draw_below(CROWD_SIDE as u64) as u32, test_random.draw_below(CROWD_SIDE as u64) as u32);
        if taken_offsets.insert((x, y)) {
            let header = Header { entity_type: SHEEP, stood_cell: cell(&world, 400 + x, 400 + y), wake: test_random.draw_below(STEP.ticks()) };
            world.put_entity(header, &hungry_at(0));
        }
    }
    let grass_before = cells_of_grass(&world);
    let mut cells_eaten = 0;
    for tick in 0..4 * STEP.ticks() {
        let report = tick_sheep(&mut world);
        assert_eq!((report.writes_applied.writes as u64, report.writes_applied.cells_changed), (report.rules[EATEN_COUNT], report.rules[EATEN_COUNT]), "tick {tick}");
        cells_eaten += report.rules[EATEN_COUNT];
    }
    assert!(cells_eaten >= 10_000, "{cells_eaten} cells eaten");
    assert_eq!(cells_of_grass(&world), grass_before - cells_eaten);
}

/// Grass and sheep together: every tick, the cells set are the spreads
/// counted and the cells cleared the decays and the meals counted
/// together, exactly -- a write is counted as it is applied, and one
/// that finds its cell changed since its rule saw it is refused, and
/// counts nothing. Two writes meet on a cell seldom: the meeting is
/// made to happen in the simulation's own tests and the sheep's
/// (`a_meal_lost_to_a_decay_leaves_the_sheep_hungry`).
#[test]
fn grass_spread_decayed_and_eaten_changes_once_a_cell() {
    let mut world = server::start_world(server::WorldStart { seed: crate::test_seed::TestSeedHandle::open().seed(), ..plain_start(1, worldgen::FRACTION_ONE / 2, 20_000, 2) });
    let chosen_rules = || server::ChosenRules::from_rules(&[server::GRASS_RULE, server::SHEEP_RULE]);
    for tick in 0..600 {
        let grass_before = cells_of_grass(&world);
        let report = world.tick_only(chosen_rules(), false);
        let (grass_counts, sheep_counts) = (report.rules.counts_of(server::GRASS_RULE), report.rules.counts_of(server::SHEEP_RULE));
        let (cells_set, cells_cleared) = set_and_cleared(report.writes_applied.cells_changed, cells_of_grass(&world) as i64 - grass_before as i64);
        assert_eq!(cells_set, grass_counts[grass::SPREADS_COUNT], "tick {tick}: cells set, spreads counted");
        let (grass_decays, cells_eaten) = (grass_counts[grass::DECAYS_COUNT], sheep_counts[EATEN_COUNT]);
        assert_eq!(cells_cleared, grass_decays + cells_eaten, "tick {tick}: cells cleared, {grass_decays} decayed and {cells_eaten} eaten");
        assert_eq!(report.writes_applied.writes as u64, report.writes_applied.cells_changed + report.writes_applied.writes_refused, "tick {tick}: a write changes its cell, or is refused");
    }
}

/// Trees alone: every tick, the trees there are grow by exactly those
/// counted put, less those that died -- two put on one cell are one
/// tree, and one put counted.
#[test]
fn a_tree_put_twice_is_one_tree() {
    let mut world = server::start_world(server::WorldStart { seed: crate::test_seed::TestSeedHandle::open().seed(), ..plain_start(1, 0, 0, 2) });
    let superchunk = first_superchunk(&world);
    // A tree every fourth cell each way, each old enough to spread and not crowded.
    let trees: Vec<CellIndex> = (0..256).flat_map(|y| (0..256).map(move |x| (4 * x, 4 * y))).map(|(x, y)| cell(&world, x, y)).collect();
    let put = world.write_cells(TREE, trees.iter().map(|&tree_cell| Write::cell(tree_cell, bitplane_manager::WriteOp::SetCell)));
    let aged_writes = world.write_cells(TREE_STAGE.layer_type(), trees.iter().map(|&aged_cell| Write::value(TREE_STAGE, aged_cell, trees::SEEDS_FROM)));
    assert_eq!((put.missed, aged_writes.missed), (0, 0));
    let trees_there = |world: &World| world.bitmap_arena().superchunk_count(TREE, superchunk) as u64;
    let mut put = 0;
    for tick in 0..2_000 {
        let trees_before = trees_there(&world);
        let trees_counts = world.tick_only(server::ChosenRules::from_rules(&[server::TREES_RULE]), false).rules.counts_of(server::TREES_RULE);
        let trees_set = trees_there(&world) + trees_counts[trees::DIED_COUNT] - trees_before;
        assert_eq!(trees_set, trees_counts[trees::SPREADS_COUNT], "tick {tick}: trees more, trees put");
        put += trees_set;
    }
    assert!(put > 0, "no tree spread");
}

/// The state of a sheep next hungry at `hungry_at`, in the middle of nothing.
fn hungry_at(hungry_at: u64) -> WholeAttribute {
    WholeAttribute::holding_value(SHEEP_STATE, SheepState { hungry_at, ..SheepState::default() })
}

/// What `sheep_ref` keeps: every sheep has its block.
fn sheep_state_of(sheep_ref: &entity_manager::EntityRef) -> SheepState {
    sheep_ref.attribute(SHEEP_STATE).expect("a sheep's state, always")
}

/// The state of a sheep next hungry at `hungry_at` whose lamb is
/// due at tick 1: the first tick a lamb can be due at, 0 being none.
fn carrying_lamb(hungry_at: u64) -> WholeAttribute {
    WholeAttribute::holding_value(SHEEP_STATE, SheepState { hungry_at, lamb_due_at: 1, ..SheepState::default() })
}

/// Two lambs put on one cell in a tick: the second is refused, and
/// its mother with it -- pregnant still, no birth counted that was
/// none -- and bears it once a cell beside her is free.
#[test]
fn a_lamb_refused_its_cell_is_born_later() {
    let mut world = server::start_world(server::WorldStart { seed: crate::test_seed::TestSeedHandle::open().seed(), ..plain_start(1, 0, 0, 1) });
    let (x, y) = (500, 500);
    for mother_x in [x, x + 2] {
        let header = Header { entity_type: SHEEP, stood_cell: cell(&world, mother_x, y), wake: 1 };
        put_entity(&mut world, header, &carrying_lamb(1));
    }
    // Every cell beside either taken, but the one between them.
    let beside_cells: Vec<(u32, u32)> = (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|&beside_cell| ![(x, y), (x + 1, y), (x + 2, y)].contains(&beside_cell)).collect();
    let blocker_headers: Vec<Header> = beside_cells.iter().map(|&(x, y)| Header { entity_type: SHEEP, stood_cell: cell(&world, x, y), wake: NEVER_WAKES }).collect();
    for &blocker_header in &blocker_headers {
        put_entity(&mut world, blocker_header, &hungry_at(0));
    }
    let lamb_count = |world: &World| world.entity_store().iter().filter(|counted_sheep| sheep_state_of(counted_sheep).grown_at != 0).count();
    let pregnant_count = |world: &World| world.entity_store().iter().filter(|pregnant_sheep| sheep_state_of(pregnant_sheep).lamb_due_at != 0).count();
    // A tick with nobody awake: the lambs are due at the next.
    tick_sheep(&mut world);
    // Both bear on the one cell, neither with another to bear on: one lamb is made, its mother's the birth, in that tick; the other
    // is refused whole, and has no cell to try again on.
    let report = tick_sheep(&mut world);
    assert_eq!((report.rules[BIRTHS_COUNT], report.instructions_applied.puts_refused, lamb_count(&world), pregnant_count(&world)), (1, 1, 1, 1));
    // A cell freed beside each: the other's lamb is born, a step's wait on.
    for freed_blocker in &blocker_headers {
        world.remove_entity(freed_blocker);
    }
    let (mut births_counted, mut most_lambs) = (1, 1);
    for _ in 1..2 + 2 * (STEP.ticks() + STEP_JITTER.ticks()) {
        births_counted += tick_sheep(&mut world).rules[BIRTHS_COUNT];
        // Seen as it is born: a lamb may die of old age at its first wake.
        most_lambs = most_lambs.max(lamb_count(&world));
    }
    assert_eq!((births_counted, most_lambs, pregnant_count(&world)), (2, 2, 0));
}

/// Two mothers bearing in one tick, two cells free between them:
/// whichever cell each drew, both lambs are born -- at once, or, the
/// two having drawn one cell, the second a step's wait on.
#[test]
fn a_lamb_whose_cell_was_taken_is_put_the_tick_after() {
    let mut world = server::start_world(server::WorldStart { seed: crate::test_seed::TestSeedHandle::open().seed(), ..plain_start(1, 0, 0, 1) });
    let (x, y) = (500, 500);
    for mother_x in [x, x + 2] {
        let header = Header { entity_type: SHEEP, stood_cell: cell(&world, mother_x, y), wake: 1 };
        // Not hungry for long: neither walks onto the cell the other's lamb is to have.
        put_entity(&mut world, header, &carrying_lamb(5000));
    }
    // Every cell beside either taken, but two between them.
    let free_cells = [(x, y), (x + 1, y), (x + 1, y + 1), (x + 2, y)];
    for (x, y) in (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|beside_cell| !free_cells.contains(beside_cell)) {
        let header = Header { entity_type: SHEEP, stood_cell: cell(&world, x, y), wake: NEVER_WAKES };
        put_entity(&mut world, header, &hungry_at(0));
    }
    // A tick with nobody awake: the lambs are due at the next.
    tick_sheep(&mut world);
    let report = tick_sheep(&mut world);
    let lambs_made = world.entity_store().iter().filter(|grown_sheep| sheep_state_of(grown_sheep).grown_at != 0).count();
    // A lamb made is a birth counted, in the tick it is made in.
    assert_eq!((report.instructions_applied.puts_refused + lambs_made, report.rules[BIRTHS_COUNT] as usize), (2, lambs_made), "each made, or refused its cell");
    let births_counted: u64 = report.rules[BIRTHS_COUNT] + (0..2 * (STEP.ticks() + STEP_JITTER.ticks())).map(|_| tick_sheep(&mut world).rules[BIRTHS_COUNT]).sum::<u64>();
    assert_eq!(births_counted, 2);
    assert!(world.entity_store().iter().all(|any_sheep| sheep_state_of(&any_sheep).lamb_due_at == 0));
}
