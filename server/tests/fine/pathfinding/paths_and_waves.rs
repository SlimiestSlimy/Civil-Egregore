//! Paths over open ground and round what is in the way: A*'s the
//! shortest, the nearest goal picked, waves spreading a cell a step.
//!
//! `cargo test`

use crate::areas::*;
use utilities::rng::RandomSource;
use server::pathfinding::{a_star, rows_hold, nearest_goal, step_towards, steps_apart, Cell, Path, Rows, Wave, AREA_SIDE};

/// A cell of the area, drawn.
fn cell_drawn(seeded_rng: &mut RandomSource) -> Cell {
    cell(seeded_rng.draw_below(AREA_SIDE as u64) as u8, seeded_rng.draw_below(AREA_SIDE as u64) as u8)
}

/// With nothing in the way a path is as long as the cells are apart,
/// and its first step is a neighbour one step nearer.
#[test]
fn paths_over_open_ground_are_straight() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    let mut cell_pairs = vec![(cell(8, 8), cell(15, 8)), (cell(8, 8), cell(0, 0)), (cell(0, 15), cell(15, 0)), (cell(8, 8), cell(9, 9))];
    cell_pairs.extend((0..300).map(|_| (cell_drawn(&mut seeded_rng), cell_drawn(&mut seeded_rng))).filter(|(from_cell, to_cell)| from_cell != to_cell));
    for (from_cell, to_cell) in cell_pairs {
        let path = a_star(&EVERY_CELL, &NO_WALLS, from_cell, to_cell).expect("a way");
        assert_eq!(path.steps, steps_apart(from_cell, to_cell));
        assert_eq!(steps_apart(from_cell, path.first_cell), 1);
        assert_eq!(steps_apart(path.first_cell, to_cell), path.steps - 1);
    }
    assert_eq!(a_star(&EVERY_CELL, &NO_WALLS, cell(4, 4), cell(4, 4)), None, "nowhere to go");
}

/// A wall with one gap is walked round through the gap; with none,
/// there is no way.
#[test]
fn paths_go_round_what_is_in_the_way() {
    let mut passable = EVERY_CELL;
    // A wall down column 10, open at row 15 only.
    for passable_row in &mut passable[..15] {
        *passable_row &= !(1 << 10);
    }
    let (from_cell, to_cell) = (cell(8, 2), cell(12, 2));
    let path = a_star(&passable, &NO_WALLS, from_cell, to_cell).expect("through the gap");
    assert_eq!(Some(path.steps), searched_steps(&passable, &NO_WALLS, from_cell, to_cell));
    assert_eq!(path.steps, 13 + 13, "down to the gap and back up");
    passable[15] &= !(1 << 10);
    assert_eq!(a_star(&passable, &NO_WALLS, from_cell, to_cell), None, "walled off");
    assert_eq!(a_star(&passable, &NO_WALLS, from_cell, cell(10, 2)).map(|path| path.steps), Some(2), "the end need not be passable");
    assert_eq!(a_star(&[0; AREA_SIDE], &NO_WALLS, cell(1, 1), cell(2, 2)).map(|path| path.steps), Some(1), "nor the start");
}

/// Walking a path a first step at a time arrives in the steps it said,
/// over passable cells only, and A*'s path is as short as a search of
/// every cell finds -- on areas of obstacles drawn at random, two in
/// three with walls drawn between their cells too.
#[test]
fn paths_are_the_shortest_and_can_be_walked() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    let (mut paths_found, mut no_paths) = (0, 0);
    for case_index in 0..3000 {
        // About a third of the cells in the way.
        let passable: Rows = std::array::from_fn(|_| (seeded_rng.next_draw() | seeded_rng.next_draw() >> 16 & seeded_rng.next_draw()) as u16);
        let walls = if case_index % 3 == 0 { NO_WALLS } else { walls_drawn(&mut seeded_rng) };
        let (from_cell, to_cell) = (cell_drawn(&mut seeded_rng), cell_drawn(&mut seeded_rng));
        if from_cell == to_cell {
            continue;
        }
        let path = a_star(&passable, &walls, from_cell, to_cell);
        assert_eq!(path.map(|path| path.steps), searched_steps(&passable, &walls, from_cell, to_cell), "{from_cell:?} to {to_cell:?}");
        let Some(Path { steps, .. }) = path else {
            no_paths += 1;
            continue;
        };
        paths_found += 1;
        let (mut walker_cell, mut steps_walked) = (from_cell, 0);
        while walker_cell != to_cell {
            let next_cell = a_star(&passable, &walls, walker_cell, to_cell).expect("still a way").first_cell;
            assert_eq!(steps_apart(walker_cell, next_cell), 1);
            assert!(next_cell == to_cell || rows_hold(&passable, next_cell), "walked onto what is in the way");
            assert!(!walls.bars_step(walker_cell, next_cell.x as i8 - walker_cell.x as i8, next_cell.y as i8 - walker_cell.y as i8), "walked through a wall");
            (walker_cell, steps_walked) = (next_cell, steps_walked + 1);
        }
        assert_eq!(steps_walked, steps);
    }
    assert!(paths_found > 500 && no_paths > 20, "{paths_found} found, {no_paths} with no way");
}

/// The nearest cell of a mask is as near as any, never the cell asked
/// from, and those equally near are each picked in turn.
#[test]
fn the_nearest_is_picked_among_the_equally_near() {
    let from_cell = cell(8, 8);
    assert_eq!(nearest_goal(&[0; AREA_SIDE], from_cell, 0), None);
    let mut goal_rows: Rows = [0; AREA_SIDE];
    goal_rows[8] |= 1 << 8;
    assert_eq!(nearest_goal(&goal_rows, from_cell, 0), None, "its own cell is nowhere to go");
    goal_rows[8] |= 1 << 11;
    goal_rows[5] |= 1 << 8;
    goal_rows[11] |= 1 << 11;
    goal_rows[0] |= 1;
    let mut picked_cells: Vec<Cell> = (0..3).map(|pick_number| nearest_goal(&goal_rows, from_cell, pick_number).expect("goals")).collect();
    assert_eq!(nearest_goal(&goal_rows, from_cell, 3), Some(picked_cells[0]), "round and round");
    picked_cells.sort_by_key(|cell| (cell.y, cell.x));
    assert_eq!(picked_cells, [cell(8, 5), cell(11, 8), cell(11, 11)], "the three three steps away, not the one eight away");
}

/// Among goals drawn, from a cell drawn: the nearest is as near as
/// any goal but the cell itself, and picks one after another go
/// through every goal that near, each once, and round again.
#[test]
fn the_nearest_of_goals_drawn_is_as_near_as_any() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    for _ in 0..1000 {
        let mut goal_rows: Rows = [0; AREA_SIDE];
        for _ in 0..seeded_rng.draw_below(12) {
            let drawn_goal = cell_drawn(&mut seeded_rng);
            goal_rows[drawn_goal.y as usize] |= 1 << drawn_goal.x;
        }
        let from_cell = cell_drawn(&mut seeded_rng);
        let other_goals: Vec<Cell> = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).filter(|&goal_cell| rows_hold(&goal_rows, goal_cell) && goal_cell != from_cell).collect();
        let Some(least_apart) = other_goals.iter().map(|&goal_cell| steps_apart(from_cell, goal_cell)).min() else {
            assert_eq!(nearest_goal(&goal_rows, from_cell, seeded_rng.next_draw()), None, "no goal but its own cell");
            continue;
        };
        let mut as_near: Vec<Cell> = other_goals.into_iter().filter(|&goal_cell| steps_apart(from_cell, goal_cell) == least_apart).collect();
        let first_pick = seeded_rng.draw_below(1 << 32);
        let mut picked_cells: Vec<Cell> = (0..as_near.len() as u64).map(|pick_number| nearest_goal(&goal_rows, from_cell, first_pick + pick_number).expect("goals")).collect();
        assert_eq!(nearest_goal(&goal_rows, from_cell, first_pick + as_near.len() as u64), Some(picked_cells[0]), "round and round");
        picked_cells.sort_by_key(|cell| (cell.y, cell.x));
        as_near.sort_by_key(|cell| (cell.y, cell.x));
        assert_eq!(picked_cells, as_near, "from {from_cell:?}");
    }
}

/// A wave spreads a cell a step over open ground, a square about its
/// goal, and stops at what is in the way.
#[test]
fn waves_spread_a_cell_a_step() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    let mut goal_rows: Rows = [0; AREA_SIDE];
    // From the middle, from a corner, and from cells drawn.
    for goal_cell in [cell(8, 8), cell(0, 15)].into_iter().chain((0..40).map(|_| cell_drawn(&mut seeded_rng))) {
        goal_rows = [0; AREA_SIDE];
        goal_rows[goal_cell.y as usize] = 1 << goal_cell.x;
        let mut wave = Wave::from_goals(&goal_rows);
        let farthest_steps = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).map(|other_cell| steps_apart(goal_cell, other_cell)).max().expect("cells");
        for steps in 1..=farthest_steps {
            assert!(wave.spread_one_step(&EVERY_CELL, &NO_WALLS), "from {goal_cell:?}, step {steps}");
            for (x, y) in (0..16).flat_map(|y| (0..16).map(move |x| (x, y))) {
                assert_eq!(rows_hold(wave.reached_rows(), cell(x, y)), steps_apart(goal_cell, cell(x, y)) <= steps, "from {goal_cell:?}: ({x}, {y}) after {steps} steps");
            }
        }
        assert!(!wave.spread_one_step(&EVERY_CELL, &NO_WALLS), "from {goal_cell:?}: every cell reached, nothing more to spread to");
    }
    let mut walled_wave = Wave::from_goals(&goal_rows);
    assert!(!walled_wave.spread_one_step(&[0; AREA_SIDE], &NO_WALLS), "nowhere to spread");
    assert_eq!(walled_wave.reached_rows(), &goal_rows);
}

/// A walker's step by waves is a first step of a shortest path to the
/// goal nearest over what may be walked on -- as far as a search of
/// every cell finds the nearest -- and walking those steps arrives.
#[test]
fn steps_by_waves_lead_to_the_nearest_goal() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = RandomSource::new(seed);
    let (mut paths_found, mut no_paths) = (0, 0);
    for case_index in 0..3000 {
        let passable: Rows = std::array::from_fn(|_| (seeded_rng.next_draw() | seeded_rng.next_draw() >> 16 & seeded_rng.next_draw()) as u16);
        let walls = if case_index % 3 == 0 { NO_WALLS } else { walls_drawn(&mut seeded_rng) };
        // A few goals, on cells that may be walked on or not.
        let mut goal_rows: Rows = [0; AREA_SIDE];
        for _ in 0..1 + seeded_rng.next_draw() % 4 {
            goal_rows[seeded_rng.next_draw() as usize % AREA_SIDE] |= 1 << (seeded_rng.next_draw() % 16);
        }
        let from_cell = cell_drawn(&mut seeded_rng);
        // Where the walker starts is nowhere to go: no goal, so it is not walked back to.
        goal_rows[from_cell.y as usize] &= !(1 << from_cell.x);
        // Goals need not be passable to be walked onto; the waves spread from them over what is.
        let mut walkable_rows = passable;
        walkable_rows.iter_mut().zip(&goal_rows).for_each(|(walkable_row, goal_row)| *walkable_row |= goal_row);
        let every_goal = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).filter(|&goal_cell| rows_hold(&goal_rows, goal_cell));
        let expected_steps = every_goal.filter_map(|goal_cell| searched_steps(&walkable_rows, &walls, from_cell, goal_cell)).min();
        let step = step_towards(&walkable_rows, &walls, &goal_rows, from_cell, seeded_rng.next_draw());
        assert_eq!(step.map(|path| path.steps), expected_steps, "from {from_cell:?}");
        let Some(Path { steps, .. }) = step else {
            no_paths += 1;
            continue;
        };
        paths_found += 1;
        let (mut walker_cell, mut steps_walked) = (from_cell, 0);
        while !rows_hold(&goal_rows, walker_cell) {
            let next_cell = step_towards(&walkable_rows, &walls, &goal_rows, walker_cell, seeded_rng.next_draw()).expect("still a way").first_cell;
            assert_eq!(steps_apart(walker_cell, next_cell), 1);
            assert!(rows_hold(&walkable_rows, next_cell), "walked onto what is in the way");
            assert!(!walls.bars_step(walker_cell, next_cell.x as i8 - walker_cell.x as i8, next_cell.y as i8 - walker_cell.y as i8), "walked through a wall");
            (walker_cell, steps_walked) = (next_cell, steps_walked + 1);
            assert!(steps_walked <= steps, "walked further than the path");
        }
        assert_eq!(steps_walked, steps);
    }
    assert!(paths_found > 500 && no_paths > 20, "{paths_found} found, {no_paths} with no way");
}
