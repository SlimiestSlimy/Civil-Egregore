//! Two writes on one cell in a tick: what a tick's counts say against
//! the cells that changed. No two sheep eat one cell; a cell of grass
//! spread onto twice, or decayed and eaten, changes once; a tree put
//! twice is one tree; and two lambs born onto one cell are one lamb
//! (`docs/server.md`, "Two writes on one cell").

use crate::tests::{cells_of_grass, first_superchunk, plain_world, plant_grass, put_entity, tick_sheep};
use bitplane_manager::Write;
use coordinates::{CellCartesian, CellIndex};
use entity_manager::{AttributeBlock, EntityId, Header, NEVER};
use entity_rules::sheep::{BIRTHS, EATEN, HUNGRY_AT, LAMB, PREGNANT, SHEEP, STEP_TICKS};
use mc_rules::{grass, trees};
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
            world.entities.queue_put(header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
        }
    }
    world.entities.apply();
    let before = cells_of_grass(&world);
    let mut eaten = 0;
    for seed in 0..4 * STEP_TICKS {
        let report = tick_sheep(&mut world, seed);
        assert_eq!((report.writes_applied.writes as u64, report.writes_applied.changed), (report.rules[EATEN], report.rules[EATEN]), "tick {seed}");
        eaten += report.rules[EATEN];
    }
    assert!(eaten >= 10_000, "{eaten} cells eaten");
    assert_eq!(cells_of_grass(&world), before - eaten);
}

/// Grass and sheep together: every tick, no more cells are set than
/// spreads were counted, and of the cells cleared there are no fewer
/// than decayed, no fewer than were eaten, and no more than both -- a
/// cell both decayed and eaten is cleared once.
#[test]
fn grass_spread_decayed_and_eaten_changes_once_a_cell() {
    let mut world = plain_world(1, 1 << 19, 20_000, 2);
    let chosen = || server::Chosen::named(&["grass", "sheep"]);
    for seed in 0..600 {
        let before = cells_of_grass(&world);
        let report = server::tick_chosen(&mut world.simulation, &mut world.arena, &mut world.entities, seed, chosen());
        let (grass, sheep) = (report.rules.of("grass"), report.rules.of("sheep"));
        let (set, cleared) = set_and_cleared(report.writes_applied.changed, cells_of_grass(&world) as i64 - before as i64);
        assert!(set <= grass[grass::SPREADS], "tick {seed}: {set} cells set");
        let (decays, eaten) = (grass[grass::DECAYS], sheep[EATEN]);
        assert!(cleared >= decays.max(eaten) && cleared <= decays + eaten, "tick {seed}: {cleared} cells cleared, {decays} decayed, {eaten} eaten");
    }
}

/// Trees alone: every tick, the trees there are grow by no more than
/// were put, less those that died -- two put on one cell are one tree.
#[test]
fn a_tree_put_twice_is_one_tree() {
    let mut world = plain_world(1, 0, 0, 2);
    let superchunk = first_superchunk(&world);
    // A tree every fourth cell each way, each old enough to spread and not crowded.
    for (x, y) in (0..256).flat_map(|y| (0..256).map(move |x| (4 * x, 4 * y))) {
        let at = cell(&world, x, y);
        world.arena.queue(TREE, Write::cell(at, bitplane_manager::WriteOp::Set));
        world.arena.queue(TREE_STAGE.layer_type(), Write::value(TREE_STAGE, at, trees::SEEDS_FROM));
    }
    assert_eq!(world.arena.apply().missed, 0);
    let trees_there = |world: &World| world.arena.superchunk_count(TREE, superchunk) as u64;
    let mut put = 0;
    for seed in 0..2_000 {
        let before = trees_there(&world);
        let done = server::tick_chosen(&mut world.simulation, &mut world.arena, &mut world.entities, seed, server::Chosen::named(&["trees"])).rules.of("trees");
        let set = trees_there(&world) + done[trees::DIED] - before;
        assert!(set <= done[trees::SPREADS], "tick {seed}: {set} trees more, {} put", done[trees::SPREADS]);
        put += set;
    }
    assert!(put > 0, "no tree spread");
}

/// Two lambs born onto one cell in a tick are one lamb: two sheep due
/// with one free cell between them both put theirs on it, the second
/// is refused -- and both count a birth and are pregnant no more.
#[test]
fn two_lambs_born_onto_one_cell_are_one() {
    let mut world = plain_world(1, 0, 0, 1);
    let (x, y) = (500, 500);
    for (id, mother) in [x, x + 2].into_iter().enumerate() {
        let header = Header { id: EntityId(1 + id as u64), kind: SHEEP, at: cell(&world, mother, y), wake: 0 };
        put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 1), AttributeBlock::holding(PREGNANT, 0)]);
    }
    // Every cell beside either taken, but the one between them.
    let beside = (x - 1..=x + 3).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))).filter(|&at| ![(x, y), (x + 1, y), (x + 2, y)].contains(&at));
    for (id, (x, y)) in beside.enumerate() {
        let header = Header { id: EntityId(10 + id as u64), kind: SHEEP, at: cell(&world, x, y), wake: NEVER };
        put_entity(&mut world, header, &[AttributeBlock::holding(HUNGRY_AT, 0)]);
    }
    let report = tick_sheep(&mut world, 0);
    assert_eq!((report.rules[BIRTHS], report.instructions_applied.refused), (2, 1));
    let lambs: Vec<CellIndex> = world.entities.iter().filter(|sheep| sheep.attribute(LAMB).is_some()).map(|sheep| sheep.header.at).collect();
    assert_eq!(lambs, [cell(&world, x + 1, y)]);
    assert!(world.entities.iter().all(|sheep| sheep.attribute(PREGNANT).is_none()));
}
