//! Two writes on one cell in a tick: a tick's counts are the cells
//! that changed, to the cell. No two sheep eat one cell; a cell of
//! grass spread onto twice is one spread, one decayed and eaten is one
//! of the two; a tree put twice is one tree put; and of two lambs put
//! on one cell the one refused is born later
//! (`docs/server.md`, "Two writes on one cell").

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, put_entity, tick_sheep};
use bitplane_manager::Write;
use coordinates::{CellCartesian, CellIndex};
use entity_manager::{AttributeBlock, EntityId, Header, NEVER};
use entity_rules::sheep::{BEARING, BIRTHS, EATEN, HUNGRY_AT, LAMB, PREGNANT, SHEEP, STEP_JITTER, STEP_TICKS};
use sca_rules::{grass, trees};
use server::World;
use std::collections::HashSet;
use type_registry::{TREE, TREE_STAGE};
use utilities::rng::Rng;

/// The cell `(x, y)` cells from the top left of `world`.
fn cell(world: &World, x: u32, y: u32) -> CellIndex {
    let corner = first_superchunk(world).top_left().cartesian();
    CellCartesian { x: corner.x + x, y: corner.y + y }.into()
}

/// Of the cells that changed in a tick, with the grass `grew` cells
/// more after it: those set and those cleared.
fn set_and_cleared(changed: u64, grew: i64) -> (u64, u64) {
    assert_eq!((changed as i64 + grew) % 2, 0, "{changed} cells changed, {grew} more grass");
    (((changed as i64 + grew) / 2) as u64, ((changed as i64 - grew) / 2) as u64)
}

/// No two sheep eat one cell: a crowd of hungry sheep on a square of
/// grass, the sheep's rule alone -- every tick, the cells eaten are as
/// many writes, and as many cells changed.
#[test]
fn no_two_sheep_eat_one_cell() {
    const SIDE: u32 = 200;
    let mut world = plain_world(1, 0, 0, 2);
    let mut random = Rng::new(utilities::seed::counted());
    let corner = cell(&world, 400, 400).cartesian();
    plant_grass(&mut world, corner, SIDE as u8, SIDE as u8);
    let mut taken = HashSet::new();
    while taken.len() < 10_000 {
        let (x, y) = (random.below(SIDE as u64) as u32, random.below(SIDE as u64) as u32);
        if taken.insert((x, y)) {
            let header = Header { id: EntityId(random.draw()), kind: SHEEP, at: cell(&world, 400 + x, 400 + y), wake: random.below(STEP_TICKS) };
            world.put_entity(header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
        }
    }
    let before = cells_of_grass(&world);
    let mut eaten = 0;
    for tick in 0..4 * STEP_TICKS {
        let report = tick_sheep(&mut world);
        assert_eq!((report.writes_applied.writes as u64, report.writes_applied.changed), (report.rules[EATEN], report.rules[EATEN]), "tick {tick}");
        eaten += report.rules[EATEN];
    }
    assert!(eaten >= 10_000, "{eaten} cells eaten");
    assert_eq!(cells_of_grass(&world), before - eaten);
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
    let mut world = plain_world(1, 1 << 19, 20_000, 2);
    let chosen = || server::Chosen::of(&[server::GRASS_RULE, server::SHEEP_RULE]);
    for tick in 0..600 {
        let before = cells_of_grass(&world);
        let report = world.tick_only(chosen(), false);
        let (grass, sheep) = (report.rules.of(server::GRASS_RULE), report.rules.of(server::SHEEP_RULE));
        let (set, cleared) = set_and_cleared(report.writes_applied.changed, cells_of_grass(&world) as i64 - before as i64);
        assert_eq!(set, grass[grass::SPREADS], "tick {tick}: cells set, spreads counted");
        let (decays, eaten) = (grass[grass::DECAYS], sheep[EATEN]);
        assert_eq!(cleared, decays + eaten, "tick {tick}: cells cleared, {decays} decayed and {eaten} eaten");
        assert_eq!(report.writes_applied.writes as u64, report.writes_applied.changed + report.writes_applied.refused, "tick {tick}: a write changes its cell, or is refused");
    }
}

/// Trees alone: every tick, the trees there are grow by exactly those
/// counted put, less those that died -- two put on one cell are one
/// tree, and one put counted.
#[test]
fn a_tree_put_twice_is_one_tree() {
    let mut world = plain_world(1, 0, 0, 2);
    let superchunk = first_superchunk(&world);
    // A tree every fourth cell each way, each old enough to spread and not crowded.
    let trees: Vec<CellIndex> = (0..256).flat_map(|y| (0..256).map(move |x| (4 * x, 4 * y))).map(|(x, y)| cell(&world, x, y)).collect();
    let put = world.write_cells(TREE, trees.iter().map(|&at| Write::cell(at, bitplane_manager::WriteOp::Set)));
    let aged = world.write_cells(TREE_STAGE.layer_type(), trees.iter().map(|&at| Write::value(TREE_STAGE, at, trees::SEEDS_FROM)));
    assert_eq!((put.missed, aged.missed), (0, 0));
    let trees_there = |world: &World| world.arena().superchunk_count(TREE, superchunk) as u64;
    let mut put = 0;
    for tick in 0..2_000 {
        let before = trees_there(&world);
        let done = world.tick_only(server::Chosen::of(&[server::TREES_RULE]), false).rules.of(server::TREES_RULE);
        let set = trees_there(&world) + done[trees::DIED] - before;
        assert_eq!(set, done[trees::SPREADS], "tick {tick}: trees more, trees put");
        put += set;
    }
    assert!(put > 0, "no tree spread");
}

/// Two lambs put on one cell in a tick: the second is refused, and
/// its mother, who does not see it stand there the tick after, is
/// pregnant still -- no birth counted that was none -- and bears it
/// once a cell beside her is free.
#[test]
fn a_lamb_refused_its_cell_is_born_later() {
    let mut world = plain_world(1, 0, 0, 1);
    let (x, y) = (500, 500);
    for (id, mother) in [x, x + 2].into_iter().enumerate() {
        let header = Header { id: EntityId(1 + id as u64), kind: SHEEP, at: cell(&world, mother, y), wake: 0 };
        put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 1), AttributeBlock::holding(PREGNANT, 0)]);
    }
    // Every cell beside either taken, but the one between them.
    let beside: Vec<(u32, u32)> = (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|&at| ![(x, y), (x + 1, y), (x + 2, y)].contains(&at)).collect();
    let blockers: Vec<Header> = beside.iter().enumerate().map(|(id, &(x, y))| Header { id: EntityId(10 + id as u64), kind: SHEEP, at: cell(&world, x, y), wake: NEVER }).collect();
    for &blocker in &blockers {
        put_entity(&mut world, blocker, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    }
    let lambs = |world: &World| world.entities().iter().filter(|sheep| sheep.attribute(LAMB).is_some()).count();
    let pregnant = |world: &World| world.entities().iter().filter(|sheep| sheep.attribute(PREGNANT).is_some()).count();
    // Both put a lamb on the one cell, neither with another to put it on: one is made, and no birth is counted before it is seen.
    let report = tick_sheep(&mut world);
    assert_eq!((report.rules[BIRTHS], report.instructions_applied.refused, lambs(&world), pregnant(&world)), (0, 1, 1, 2));
    // The tick after, one mother sees her lamb; the other does not, and has no cell to try again on.
    let report = tick_sheep(&mut world);
    assert_eq!((report.rules[BIRTHS], lambs(&world), pregnant(&world)), (1, 1, 1));
    // A cell freed beside each: the other's lamb is born, a step's wait and a tick on.
    for blocker in &blockers {
        world.remove_entity(blocker);
    }
    let (mut births, mut most_lambs) = (1, 1);
    for _ in 2..2 + 2 * (STEP_TICKS + STEP_JITTER) {
        births += tick_sheep(&mut world).rules[BIRTHS];
        // Seen as it is born: a lamb may die of old age at its first wake.
        most_lambs = most_lambs.max(lambs(&world));
    }
    assert_eq!((births, most_lambs, pregnant(&world)), (2, 2, 0));
    assert!(world.entities().iter().all(|sheep| sheep.attribute(BEARING).is_none()));
}

/// Two mothers bearing in one tick, two cells free between them:
/// whichever cell each drew, both lambs are born -- at once, or, the
/// two having drawn one cell, the second a tick on, on the other.
#[test]
fn a_lamb_whose_cell_was_taken_is_put_the_tick_after() {
    let mut world = plain_world(1, 0, 0, 1);
    let (x, y) = (500, 500);
    for (id, mother) in [x, x + 2].into_iter().enumerate() {
        let header = Header { id: EntityId(1 + id as u64), kind: SHEEP, at: cell(&world, mother, y), wake: 0 };
        // Not hungry for long: neither walks onto the cell the other's lamb is to have.
        put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 5000), AttributeBlock::holding(PREGNANT, 0)]);
    }
    // Every cell beside either taken, but two between them.
    let free = [(x, y), (x + 1, y), (x + 1, y + 1), (x + 2, y)];
    for (id, (x, y)) in (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|at| !free.contains(at)).enumerate() {
        let header = Header { id: EntityId(10 + id as u64), kind: SHEEP, at: cell(&world, x, y), wake: NEVER };
        put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    }
    let made = tick_sheep(&mut world).instructions_applied;
    assert_eq!(made.refused + world.entities().iter().filter(|sheep| sheep.attribute(LAMB).is_some()).count(), 2, "each made, or refused its cell");
    let births: u64 = (0..4).map(|_| tick_sheep(&mut world).rules[BIRTHS]).sum();
    assert_eq!(births, 2);
    assert!(world.entities().iter().all(|sheep| sheep.attribute(PREGNANT).is_none() && sheep.attribute(BEARING).is_none()));
}
