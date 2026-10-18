//! Walking: the way to a cell goes round what is in the way, a cell
//! of a layer is sought further and further off, the terrain's walls
//! bar steps, and what bars one bars a path.
//!
//! `cargo test --test fast`

// TEMPORARY: reaches the server's internals -- `server::internals_for_root_tests`
// and the world's handles to them -- until the world has reads of its
// own and this is rewritten against them (AI.txt, LATER).

use crate::worlds::base;
use crate::arenas::{cell, walker_header, world, STONE_LAYER};
use server::internals_for_root_tests::{BitmapArena, BucketKey, Write, WriteOp};
use server::internals_for_root_tests::LayerCodec;
use utilities::coordinates::{SuperchunkIndex};
use server::simulation::simulation_api::square_around_a_cell::{self, AREA_CENTRE};
use server::simulation::simulation_api::{cell_and_its_neighbours, entity_reads_and_writes, steps_and_paths};
use server::internals_for_root_tests::NEVER_WAKES;
use server::simulation::simulation_api::SimulationTickState;
use server::internals_for_root_tests::Simulation;
use basegame::libraries::collision::Collision;
use std::sync::Mutex;

/// What is in a walker's way here: the base game's collision plane and walls.
fn in_the_way() -> steps_and_paths::PlanesInTheWay {
    steps_and_paths::PlanesInTheWay { collision: base().collision, wall_north: base().wall_north, wall_west: base().wall_west, wall_south: base().wall_south, wall_east: base().wall_east }
}

/// Makes the wall layers hot over the superchunk `(10, 10)` of `bitmap_arena`.
fn with_walls_hot(bitmap_arena: &mut BitmapArena) {
    let mut layer_codec = LayerCodec::new();
    for layer_type in [base().wall_north, base().wall_west, base().wall_south, base().wall_east] {
        for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type, chunk }, None, &mut layer_codec);
        }
    }
}

/// The way to a cell is found a step at a time, round what cannot be
/// walked on and round the entities in the way; a cell out of the area
/// has none.
#[test]
fn a_step_to_a_cell_goes_round_what_is_in_the_way() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    let (start_cell, goal_cell) = (cell(40, 30), cell(44, 30));
    entity_store.queue_add(walker_header(start_cell, 0), &[]);
    // A wall of entities across the straight way, open only below.
    for y in 23..=30 {
        entity_store.queue_add(walker_header(cell(42, y), NEVER_WAKES), &[]);
    }
    entity_store.apply();
    // The entities are kept in the collision plane, as a world keeps them: each put there holds its cell.
    let mut layer_codec = LayerCodec::new();
    for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
        bitmap_arena.make_hot(BucketKey { layer_type: base().collision, chunk }, None, &mut layer_codec);
    }
    entity_store.iter().for_each(|entity| bitmap_arena.queue_write(base().collision, Write::cell(entity.header.stood_cell, WriteOp::SetCell)));
    bitmap_arena.apply();
    let mut test_simulation = Simulation::new(1);
    let mut steps = 0;
    while entity_store.get(goal_cell).is_none() {
        test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
            let tick_now = tick_state.current_tick();
            for entity in tick_state.woken_entities() {
                let walker_cell = entity.header.stood_cell;
                let passable = square_around_a_cell::read_layer(tick_state, STONE_LAYER, walker_cell).hot;
                assert_eq!(steps_and_paths::step_to(tick_state, in_the_way(), walker_cell, walker_cell.at_cartesian_offset(9, 0).unwrap(), &passable), None, "out of the area");
                let next_cell = steps_and_paths::step_to(tick_state, in_the_way(), walker_cell, goal_cell, &passable).expect("a way round");
                Collision { plane: base().collision }.step(tick_state, &entity.header, next_cell, tick_now + 1);
            }
            0
        });
        steps += 1;
        assert!(steps <= 8, "no way found in the steps it takes");
    }
    // Down past the wall's end and up again: two cells across, one down, one up, the diagonals counted once.
    assert_eq!(steps, 4);
}

/// What the collision plane holds is not stepped on: a neighbour it
/// holds is not among those a step may be taken to, and the way to a
/// cell goes round a row of them as it does round entities.
#[test]
fn a_cell_the_collision_plane_holds_is_not_stepped_on() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    let mut layer_codec = LayerCodec::new();
    for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
        bitmap_arena.make_hot(BucketKey { layer_type: base().collision, chunk }, None, &mut layer_codec);
    }
    let (start_cell, goal_cell) = (cell(40, 30), cell(44, 30));
    // Trees, say, across the straight way: open only below.
    for y in 23..=30 {
        bitmap_arena.queue_write(base().collision, Write::cell(cell(42, y), WriteOp::SetCell));
    }
    bitmap_arena.queue_write(base().collision, Write::cell(cell(41, 29), WriteOp::SetCell));
    bitmap_arena.apply();
    entity_store.queue_add(walker_header(start_cell, 0), &[]);
    entity_store.apply();
    let mut test_simulation = Simulation::new(1);
    let mut steps = 0;
    while entity_store.get(goal_cell).is_none() {
        test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
            let tick_now = tick_state.current_tick();
            for entity in tick_state.woken_entities() {
                let walker_cell = entity.header.stood_cell;
                if walker_cell == start_cell {
                    // The neighbour up and to the east is held, the others are free.
                    let about_tiles = cell_and_its_neighbours::tiles_about_cell(walker_cell);
                    assert_eq!(steps_and_paths::around_steppable(tick_state, in_the_way(), &about_tiles), about_tiles.neighbour_cells().cells_not_in(about_tiles.cell_towards(cell_and_its_neighbours::way_of(walker_cell, cell(41, 29)))));
                }
                let next_cell = steps_and_paths::step_to(tick_state, in_the_way(), walker_cell, goal_cell, &square_around_a_cell::read_layer(tick_state, STONE_LAYER, walker_cell).hot).expect("a way round");
                assert_eq!(tick_state.layer_holds(base().collision, next_cell), Ok(false), "a step onto what the collision plane holds");
                Collision { plane: base().collision }.step(tick_state, &entity.header, next_cell, tick_now + 1);
            }
            0
        });
        steps += 1;
        assert!(steps <= 8, "no way found in the steps it takes");
    }
    assert_eq!(steps, 4);
}

/// A cell is sought further and further off, over tiles twice the side
/// each time: the scale it is found at is the first whose tiles reach
/// it, the step is towards it, and with none in reach there is no step.
#[test]
fn a_cell_is_sought_further_and_further_off() {
    let (mut bitmap_arena, mut entity_store) = world(2);
    let start_cell = cell(1000, 1000);
    entity_store.queue_add(walker_header(start_cell, 0), &[]);
    entity_store.apply();
    let mut test_simulation = Simulation::new(1);
    // How far off the one cell set is, across and down, and the scale its tile is first seen at.
    let offset_cases = [(0, 0, None), (5, -3, Some(0)), (-12, 4, Some(1)), (20, 20, Some(2)), (3, -50, Some(3)), (-100, 90, Some(4)), (200, 10, Some(5)), (-40, 400, Some(6)), (-300, -450, Some(6))];
    for (seed, (goal_across, goal_down, scale)) in offset_cases.into_iter().enumerate() {
        let goal_cell = start_cell.at_cartesian_offset(goal_across, goal_down).expect("in the world");
        if scale.is_some() {
            bitmap_arena.queue_write(STONE_LAYER, Write::cell(goal_cell, WriteOp::SetCell));
            bitmap_arena.apply();
        }
        test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed as u64, |tick_state: &mut SimulationTickState| {
            for entity in tick_state.woken_entities() {
                let found_step = steps_and_paths::seek(tick_state, in_the_way(), start_cell, STONE_LAYER);
                assert_eq!(found_step.map(|mapped_step| mapped_step.scale), scale, "{goal_across} across, {goal_down} down");
                if let Some(some_step) = found_step {
                    let (step_cartesian, start_cartesian, goal_cartesian) = (some_step.step_cell.cartesian(), start_cell.cartesian(), goal_cell.cartesian());
                    assert_eq!(step_cartesian.x.abs_diff(goal_cartesian.x).max(step_cartesian.y.abs_diff(goal_cartesian.y)) + 1, start_cartesian.x.abs_diff(goal_cartesian.x).max(start_cartesian.y.abs_diff(goal_cartesian.y)), "a step nearer");
                }
                let wake = tick_state.current_tick() + 1;
                entity_reads_and_writes::sleep(tick_state, &entity.header, wake);
            }
            0
        });
        if scale.is_some() {
            bitmap_arena.queue_write(STONE_LAYER, Write::cell(goal_cell, WriteOp::ClearCell));
            bitmap_arena.apply();
        }
    }
}

/// The terrain's walls bar steps: the cells about an entity that may be
/// stepped to leave out those a wall is before, and the way to a cell
/// goes round a cliff, through its one gap.
#[test]
fn walls_of_the_terrain_bar_steps() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    with_walls_hot(&mut bitmap_arena);
    // A cliff between columns 41 and 42, rows 20 to 40, with a gap at row 33: walls east of column 41, which bar the diagonals across it too.
    for y in (20..=40).filter(|&y| y != 33) {
        bitmap_arena.queue_write(base().wall_east, Write::cell(cell(41, y), WriteOp::SetCell));
    }
    bitmap_arena.apply();
    let (start_cell, goal_cell) = (cell(41, 30), cell(43, 30));
    entity_store.queue_add(walker_header(start_cell, 0), &[]);
    entity_store.apply();
    let mut test_simulation = Simulation::new(1);
    let mut steps = 0;
    while entity_store.get(goal_cell).is_none() {
        test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
            let tick_now = tick_state.current_tick();
            for entity in tick_state.woken_entities() {
                let walker_cell = entity.header.stood_cell;
                if walker_cell == start_cell {
                    // The three cells east of it are behind the cliff.
                    let about_start_tiles = cell_and_its_neighbours::tiles_about_cell(walker_cell);
                    assert_eq!(steps_and_paths::around_unwalled(tick_state, in_the_way(), &about_start_tiles), about_start_tiles.nine_cells().cells_not_in(about_start_tiles.cell_towards(cell_and_its_neighbours::NORTH_EAST) | about_start_tiles.cell_towards(cell_and_its_neighbours::EAST) | about_start_tiles.cell_towards(cell_and_its_neighbours::SOUTH_EAST)));
                }
                let passable = square_around_a_cell::read_layer(tick_state, STONE_LAYER, walker_cell).hot;
                let next_cell = steps_and_paths::step_to(tick_state, in_the_way(), walker_cell, goal_cell, &passable).expect("a way through the gap");
                let about_tiles = cell_and_its_neighbours::tiles_about_cell(walker_cell);
                assert!(!(steps_and_paths::around_unwalled(tick_state, in_the_way(), &about_tiles) & about_tiles.cell_towards(cell_and_its_neighbours::way_of(walker_cell, next_cell))).is_empty(), "a step through a wall");
                server::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &entity.header, next_cell, tick_now + 1);
            }
            0
        });
        steps += 1;
        assert!(steps <= 12, "no way found in the steps it takes");
    }
    // Down to the gap at row 33, straight through it -- a diagonal across the cliff has a wall on one way round -- and back up: three down, one across, three up.
    assert_eq!(steps, 7);
}

/// A drop is stepped down and not back up: with no step west out of
/// a column, a row drawn from the seed is crossed eastwards straight
/// over it, and westwards there is no way in the area -- nor is the
/// cell above among those that may be stepped to from below.
#[test]
fn a_drop_is_stepped_down_and_not_back_up() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    with_walls_hot(&mut bitmap_arena);
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    let cliff_row = 20 + seeded_random.draw_below(20) as u32;
    // High ground west of column 42: every cell of the column bars its step west, further up and down than an area reaches.
    for y in cliff_row - 16..=cliff_row + 16 {
        bitmap_arena.queue_write(base().wall_west, Write::cell(cell(42, y), WriteOp::SetCell));
    }
    bitmap_arena.apply();
    let (above_cell, below_cell) = (cell(41, cliff_row), cell(43, cliff_row));
    Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        let (about_above, about_foot) = (cell_and_its_neighbours::tiles_about_cell(above_cell), cell_and_its_neighbours::tiles_about_cell(cell(42, cliff_row)));
        assert_eq!(steps_and_paths::around_unwalled(tick_state, in_the_way(), &about_above), about_above.nine_cells(), "every step open from above");
        let west_ways = about_foot.cell_towards(cell_and_its_neighbours::NORTH_WEST) | about_foot.cell_towards(cell_and_its_neighbours::WEST) | about_foot.cell_towards(cell_and_its_neighbours::SOUTH_WEST);
        assert_eq!(steps_and_paths::around_unwalled(tick_state, in_the_way(), &about_foot), about_foot.nine_cells().cells_not_in(west_ways), "none back up");
        let passable = square_around_a_cell::read_layer(tick_state, STONE_LAYER, above_cell).hot;
        assert_eq!(steps_and_paths::step_to(tick_state, in_the_way(), above_cell, below_cell, &passable).map(|next_cell| next_cell.cartesian().x), Some(cell(42, cliff_row).cartesian().x), "down it");
        let passable = square_around_a_cell::read_layer(tick_state, STONE_LAYER, below_cell).hot;
        assert_eq!(steps_and_paths::step_to(tick_state, in_the_way(), below_cell, above_cell, &passable), None, "no way back in the area");
        0
    });
}

/// The two readings of the walls agree, on walls set at random: the
/// steps out of a cell the turn leaves open ([`walking::around_unwalled`])
/// are those pathfinding's walls about it do not bar
/// (`Walls::bars_step`), diagonals included -- a diagonal open only when
/// both ways round it are.
#[test]
fn the_turn_and_pathfinding_bar_the_same_steps() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut bitmap_arena, mut entity_store) = world(1);
    with_walls_hot(&mut bitmap_arena);
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    for y in 0..64 {
        for x in 0..64 {
            for layer_type in [base().wall_north, base().wall_west, base().wall_south, base().wall_east] {
                if seeded_random.draw_below(4) == 0 {
                    bitmap_arena.queue_write(layer_type, Write::cell(cell(x, y), WriteOp::SetCell));
                }
            }
        }
    }
    bitmap_arena.apply();
    let checked_count = Mutex::new(0);
    Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        for y in 1..63 {
            for x in 1..63 {
                let asked_cell = cell(x, y);
                let (unwalled_cells, walls) = (steps_and_paths::around_unwalled(tick_state, in_the_way(), &cell_and_its_neighbours::tiles_about_cell(asked_cell)), steps_and_paths::area_walls(tick_state, in_the_way(), asked_cell));
                let area_centre = server::internals_for_root_tests::Cell { x: AREA_CENTRE as u8, y: AREA_CENTRE as u8 };
                for way_checked in (0..cell_and_its_neighbours::WAYS).filter(|&way_filtered| way_filtered != cell_and_its_neighbours::HERE) {
                    let beside_cartesian = cell_and_its_neighbours::cell(asked_cell, way_checked).expect("in the world").cartesian();
                    let (dx, dy) = ((beside_cartesian.x as i64 - x as i64) as i8, (beside_cartesian.y as i64 - y as i64) as i8);
                    assert_eq!(!(unwalled_cells & cell_and_its_neighbours::tiles_about_cell(asked_cell).cell_towards(way_checked)).is_empty(), !walls.bars_step(area_centre, dx, dy), "({x}, {y}) by ({dx}, {dy})");
                    *checked_count.lock().unwrap() += 1;
                }
            }
        }
        0
    });
    assert_eq!(checked_count.into_inner().unwrap(), 62 * 62 * 8);
}
