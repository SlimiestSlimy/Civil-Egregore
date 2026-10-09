//! Walking: the way to a cell goes round what is in the way, a cell
//! of a layer is sought further and further off, the terrain's walls
//! bar steps, and what bars one bars a path.
//!
//! `cargo test --test fast`

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use instructions::read::{area, walking};
use instructions::write::entities;
use instructions::around;
use instructions::area::AREA_CENTRE;
use entity_manager::{Entities, EntityId, EntityType, Header, NEVER};
use instructions::{Simulation, Turn};
use std::sync::Mutex;
use worldgen::{WALL_EAST, WALL_SOUTH};

/// The layer type the arena holds: every cell hot, none set.
const STONE: LayerType = LayerType(6);
/// The entities' type.
const WALKER: EntityType = EntityType(40);

/// An arena with a bitmap hot over the `side` by `side` superchunks from
/// `(10, 10)`, and entities holding the same superchunks.
fn world(side: u32) -> (BitmapArena, Entities) {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side {
        for x in 10..10 + side {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
            }
        }
    }
    let mut entities = Entities::new();
    assert_eq!(entities.align(&arena.superchunk_indices()), 0);
    (arena, entities)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker with ID `id` on `at`, waking at `wake`.
fn walker(id: u64, at: CellIndex, wake: u64) -> Header {
    Header { id: EntityId(id), kind: WALKER, at, wake }
}

/// The way to a cell is found a step at a time, round what cannot be
/// walked on and round the entities in the way; a cell out of the area
/// has none.
#[test]
fn a_step_to_a_cell_goes_round_what_is_in_the_way() {
    let (mut arena, mut entities) = world(1);
    let (from, to) = (cell(40, 30), cell(44, 30));
    entities.queue_put(walker(1, from, 0), &[]);
    // A wall of entities across the straight way, open only below.
    for y in 23..=30 {
        entities.queue_put(walker(10 + y as u64, cell(42, y), NEVER), &[]);
    }
    entities.apply();
    let mut simulation = Simulation::new(1);
    let mut steps = 0;
    while entities.get(EntityId(1), to).is_none() {
        simulation.tick(&mut arena, &mut entities, steps, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            let now = turn.now();
            for entity in turn.woken() {
                let at = entity.header.at;
                let passable = area::layer(turn, STONE, at).hot;
                assert_eq!(walking::step_to(turn, at, at.offset(9, 0).unwrap(), &passable), None, "out of the area");
                let next = walking::step_to(turn, at, to, &passable).expect("a way round");
                turn.step(&entity.header, next, now + 1);
            }
            0
        });
        steps += 1;
        assert!(steps <= 8, "no way found in the steps it takes");
    }
    // Down past the wall's end and up again: two cells across, one down, one up, the diagonals counted once.
    assert_eq!(steps, 4);
}

/// A cell is sought further and further off, over tiles twice the side
/// each time: the scale it is found at is the first whose tiles reach
/// it, the step is towards it, and with none in reach there is no step.
#[test]
fn a_cell_is_sought_further_and_further_off() {
    let (mut arena, mut entities) = world(2);
    let from = cell(1000, 1000);
    entities.queue_put(walker(1, from, 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    // How far off the one cell set is, across and down, and the scale its tile is first seen at.
    let cases = [(0, 0, None), (5, -3, Some(0)), (-12, 4, Some(1)), (20, 20, Some(2)), (3, -50, Some(3)), (-100, 90, Some(4)), (200, 10, Some(5)), (-40, 400, Some(6)), (-300, -450, Some(6))];
    for (seed, (across, down, scale)) in cases.into_iter().enumerate() {
        let goal = from.offset(across, down).expect("in the world");
        if scale.is_some() {
            arena.queue(STONE, Write::cell(goal, WriteOp::Set));
            arena.apply();
        }
        simulation.tick(&mut arena, &mut entities, seed as u64, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                let found = walking::seek(turn, from, STONE);
                assert_eq!(found.map(|found| found.scale), scale, "{across} across, {down} down");
                if let Some(found) = found {
                    let (to, at, goal) = (found.to.cartesian(), from.cartesian(), goal.cartesian());
                    assert_eq!(to.x.abs_diff(goal.x).max(to.y.abs_diff(goal.y)) + 1, at.x.abs_diff(goal.x).max(at.y.abs_diff(goal.y)), "a step nearer");
                }
                let wake = turn.now() + 1;
                entities::sleep(turn, &entity.header, wake);
            }
            0
        });
        if scale.is_some() {
            arena.queue(STONE, Write::cell(goal, WriteOp::Unset));
            arena.apply();
        }
    }
}

/// The terrain's walls bar steps: the cells about an entity that may be
/// stepped to leave out those a wall is before, and the way to a cell
/// goes round a cliff, through its one gap.
#[test]
fn walls_of_the_terrain_bar_steps() {
    let (mut arena, mut entities) = world(1);
    let mut codec = LayerCodec::new();
    for layer_type in [WALL_EAST, WALL_SOUTH] {
        for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
            arena.make_hot(BucketKey { layer_type, chunk }, None, &mut codec);
        }
    }
    // A cliff between columns 41 and 42, rows 20 to 40, with a gap at row 33: walls east of column 41, which bar the diagonals across it too.
    for y in (20..=40).filter(|&y| y != 33) {
        arena.queue(WALL_EAST, Write::cell(cell(41, y), WriteOp::Set));
    }
    arena.apply();
    let (from, to) = (cell(41, 30), cell(43, 30));
    entities.queue_put(walker(1, from, 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    let mut steps = 0;
    while entities.get(EntityId(1), to).is_none() {
        simulation.tick(&mut arena, &mut entities, steps, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            let now = turn.now();
            for entity in turn.woken() {
                let at = entity.header.at;
                if at == from {
                    // The three cells east of it are behind the cliff.
                    assert_eq!(walking::around_unwalled(turn, at), around::ALL & !(1 << 2 | 1 << 5 | 1 << 8));
                }
                let passable = area::layer(turn, STONE, at).hot;
                let next = walking::step_to(turn, at, to, &passable).expect("a way through the gap");
                assert!(walking::around_unwalled(turn, at) >> around::bit_of(at, next) & 1 == 1, "a step through a wall");
                turn.step(&entity.header, next, now + 1);
            }
            0
        });
        steps += 1;
        assert!(steps <= 12, "no way found in the steps it takes");
    }
    // Down to the gap at row 33, straight through it -- a diagonal across the cliff has a wall on one way round -- and back up: three down, one across, three up.
    assert_eq!(steps, 7);
}

/// The two readings of the walls agree, on walls set at random: the
/// steps out of a cell the turn leaves open ([`walking::around_unwalled`])
/// are those pathfinding's walls about it do not bar
/// (`Walls::bars_step`), diagonals included -- a diagonal open only when
/// both ways round it are.
#[test]
fn the_turn_and_pathfinding_bar_the_same_steps() {
    let (mut arena, mut entities) = world(1);
    let mut codec = LayerCodec::new();
    for layer_type in [WALL_EAST, WALL_SOUTH] {
        for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
            arena.make_hot(BucketKey { layer_type, chunk }, None, &mut codec);
        }
    }
    let mut random = utilities::rng::Rng::new(7);
    for y in 0..64 {
        for x in 0..64 {
            for layer_type in [WALL_EAST, WALL_SOUTH] {
                if random.below(4) == 0 {
                    arena.queue(layer_type, Write::cell(cell(x, y), WriteOp::Set));
                }
            }
        }
    }
    arena.apply();
    let checked = Mutex::new(0);
    Simulation::new(1).tick(&mut arena, &mut entities, 0, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        for y in 1..63 {
            for x in 1..63 {
                let at = cell(x, y);
                let (open, walls) = (walking::around_unwalled(turn, at), walking::area_walls(turn, at));
                let centre = pathfinding::Cell { x: AREA_CENTRE as u8, y: AREA_CENTRE as u8 };
                for bit in (0..9).filter(|&bit| bit != 4) {
                    let (dx, dy) = (bit as i8 % 3 - 1, bit as i8 / 3 - 1);
                    assert_eq!(open >> bit & 1 == 1, !walls.bars_step(centre, dx, dy), "({x}, {y}) by ({dx}, {dy})");
                    *checked.lock().unwrap() += 1;
                }
            }
        }
        0
    });
    assert_eq!(checked.into_inner().unwrap(), 62 * 62 * 8);
}
