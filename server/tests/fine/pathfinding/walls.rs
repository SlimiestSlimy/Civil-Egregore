//! Walls between cells: barring a step both ways or one way alone,
//! one thick in any direction, never crossed by waves, by A* or by a
//! walker.
//!
//! `cargo test`

use crate::areas::*;
use utilities::rng::RandomSource;
use server::pathfinding::{a_star, rows_hold, step_towards, Cell, Rows, Walls, Wave, AREA_SIDE};

/// A wall bars the step between two cells, both ways, whatever the cells
/// are: a wall across the area with one gap is gone round by waves and
/// by A* alike, and one with none is not crossed; a diagonal step is
/// barred unless both ways round it are open.
#[test]
fn walls_bar_steps_between_cells() {
    // A wall under row 7, all the way across.
    let mut south_walls = [0; AREA_SIDE];
    south_walls[7] = u16::MAX;
    let walls = Walls::new([0; AREA_SIDE], south_walls);
    let (from_cell, to_cell) = (cell(3, 5), cell(3, 10));
    let mut goal_rows: Rows = [0; AREA_SIDE];
    goal_rows[to_cell.y as usize] = 1 << to_cell.x;
    assert_eq!(a_star(&EVERY_CELL, &walls, from_cell, to_cell), None, "no way through");
    assert_eq!(a_star(&EVERY_CELL, &walls, to_cell, from_cell), None, "nor back");
    assert_eq!(step_towards(&EVERY_CELL, &walls, &goal_rows, from_cell, 0), None);

    // A gap at column 12: the straight step down alone, the diagonals through it each walled on a way round.
    south_walls[7] &= !(1 << 12);
    let walls = Walls::new([0; AREA_SIDE], south_walls);
    let path = a_star(&EVERY_CELL, &walls, from_cell, to_cell).expect("through the gap");
    assert_eq!(path.steps, 9 + 1 + 9, "nine across to the gap's column, down through it, and nine back");
    let wave = step_towards(&EVERY_CELL, &walls, &goal_rows, from_cell, 0).expect("through the gap");
    assert_eq!(wave.steps, path.steps, "waves and A* agree");
    // Walked a step at a time, it gets there in as many steps, never through the wall.
    let (mut walker_cell, mut steps_taken) = (from_cell, 0);
    while walker_cell != to_cell {
        let next_cell = step_towards(&EVERY_CELL, &walls, &goal_rows, walker_cell, steps_taken).expect("still a way").first_cell;
        assert!(!walls.bars_step(walker_cell, next_cell.x as i8 - walker_cell.x as i8, next_cell.y as i8 - walker_cell.y as i8), "{walker_cell:?} to {next_cell:?} through a wall");
        (walker_cell, steps_taken) = (next_cell, steps_taken + 1);
    }
    assert_eq!(steps_taken, path.steps as u64);

    // One wall, east of (4, 4): it bars the step across it and every diagonal with it on a way round, both ways.
    let mut east_walls = [0; AREA_SIDE];
    east_walls[4] = 1 << 4;
    let corner_walls = Walls::new(east_walls, [0; AREA_SIDE]);
    for (start_cartesian, (dx, dy)) in [((4, 4), (1, 0)), ((4, 4), (1, 1)), ((5, 4), (-1, 1)), ((4, 3), (1, 1)), ((5, 3), (-1, 1)), ((4, 4), (1, -1)), ((5, 5), (-1, -1))] {
        assert!(corner_walls.bars_step(cell(start_cartesian.0, start_cartesian.1), dx, dy), "{start_cartesian:?} by ({dx}, {dy})");
    }
    for (start_cartesian, (dx, dy)) in [((4, 4), (0, 1)), ((4, 4), (0, -1)), ((4, 4), (-1, 1)), ((5, 4), (1, 1)), ((5, 4), (0, 1))] {
        assert!(!corner_walls.bars_step(cell(start_cartesian.0, start_cartesian.1), dx, dy), "{start_cartesian:?} by ({dx}, {dy})");
    }
    assert_eq!(a_star(&EVERY_CELL, &corner_walls, cell(4, 4), cell(5, 4)).map(|path| path.steps), Some(3), "round the wall's end, the diagonals barred");
}

/// The walls between cells of different heights, as the terrain puts
/// them: east and south of a cell more than one apart from its neighbour.
fn walls_of(height: impl Fn(usize, usize) -> u8) -> Walls {
    let (mut east_walls, mut south_walls) = ([0; AREA_SIDE], [0; AREA_SIDE]);
    for y in 0..AREA_SIDE {
        for x in 0..AREA_SIDE {
            if x + 1 < AREA_SIDE && height(x, y).abs_diff(height(x + 1, y)) > 1 {
                east_walls[y] |= 1 << x;
            }
            if y + 1 < AREA_SIDE && height(x, y).abs_diff(height(x, y + 1)) > 1 {
                south_walls[y] |= 1 << x;
            }
        }
    }
    Walls::new(east_walls, south_walls)
}

/// The cells reached from `start_cell` by waves over every cell, through no
/// wall.
fn reached_from(start_cell: Cell, walls: &Walls) -> Rows {
    let mut goal_rows: Rows = [0; AREA_SIDE];
    goal_rows[start_cell.y as usize] = 1 << start_cell.x;
    let mut wave = Wave::from_goals(&goal_rows);
    while wave.spread_one_step(&EVERY_CELL, walls) {}
    *wave.reached_rows()
}

/// A wall one thick, in any direction -- across, down, or diagonal, as
/// a cliff's edge or as a ridge of single cells -- is never crossed: by
/// waves, by A*, or by a walker stepping towards the far side. A
/// diagonal step across it always has a wall on one of its two ways
/// round, so it is barred.
#[test]
fn walls_one_thick_in_any_direction_are_never_crossed() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    // Each case: the heights, and which side of the wall a cell is on (`None` on the ridge itself).
    type Height = fn(usize, usize) -> u8;
    type SideOfWall = fn(usize, usize) -> Option<bool>;
    let wall_cases: [(&str, Height, SideOfWall); 6] = [
        ("a cliff down the middle", |x, _| if x < 8 { 9 } else { 0 }, |x, _| Some(x < 8)),
        ("a cliff across the middle", |_, y| if y < 8 { 9 } else { 0 }, |_, y| Some(y < 8)),
        ("a cliff along the diagonal", |x, y| if x > y { 9 } else { 0 }, |x, y| Some(x > y)),
        ("a cliff along the other diagonal", |x, y| if x + y < AREA_SIDE { 9 } else { 0 }, |x, y| Some(x + y < AREA_SIDE)),
        ("a ridge one cell thick along the diagonal", |x, y| if x == y { 9 } else { 0 }, |x, y| (x != y).then_some(x > y)),
        ("a ridge one cell thick along the other diagonal", |x, y| if x + y == AREA_SIDE - 1 { 9 } else { 0 }, |x, y| (x + y != AREA_SIDE - 1).then_some(x + y < AREA_SIDE - 1)),
    ];
    for (case_name, height, side_of_cell) in wall_cases {
        let walls = walls_of(height);
        let cells = || (0..AREA_SIDE).flat_map(|y| (0..AREA_SIDE).map(move |x| (x, y)));
        // Every cell of a side reaches every other of its side, and none of the other's, nor the ridge.
        for (x, y) in cells() {
            let Some(here_side) = side_of_cell(x, y) else {
                continue;
            };
            let reached_rows = reached_from(cell(x as u8, y as u8), &walls);
            for (other_x, other_y) in cells() {
                let got_there = rows_hold(&reached_rows, cell(other_x as u8, other_y as u8));
                assert_eq!(got_there, side_of_cell(other_x, other_y) == Some(here_side), "{case_name}: ({x}, {y}) to ({other_x}, {other_y})");
            }
        }
        // A* and a walker find no way across, from either side.
        let mut seeded_rng = RandomSource::new(seed);
        for _ in 0..200 {
            let draw_cell = |seeded_rng: &mut RandomSource| cell((seeded_rng.next_draw() % AREA_SIDE as u64) as u8, (seeded_rng.next_draw() % AREA_SIDE as u64) as u8);
            let (from_cell, to_cell) = (draw_cell(&mut seeded_rng), draw_cell(&mut seeded_rng));
            let (Some(from_side), Some(to_side)) = (side_of_cell(from_cell.x as usize, from_cell.y as usize), side_of_cell(to_cell.x as usize, to_cell.y as usize)) else {
                continue;
            };
            if from_side == to_side || from_cell == to_cell {
                continue;
            }
            assert_eq!(a_star(&EVERY_CELL, &walls, from_cell, to_cell), None, "{case_name}: A* from {from_cell:?} to {to_cell:?}");
            let mut goal_rows: Rows = [0; AREA_SIDE];
            goal_rows[to_cell.y as usize] = 1 << to_cell.x;
            assert_eq!(step_towards(&EVERY_CELL, &walls, &goal_rows, from_cell, seeded_rng.next_draw()), None, "{case_name}: a walker from {from_cell:?} to {to_cell:?}");
        }
        // No step between neighbours on two sides is open, diagonals included.
        for (x, y) in cells() {
            for (dx, dy) in [(1i8, 0i8), (0, 1), (1, 1), (-1, 1)] {
                let (other_x, other_y) = (x as i8 + dx, y as i8 + dy);
                if other_x < 0 || other_x >= AREA_SIDE as i8 || other_y >= AREA_SIDE as i8 {
                    continue;
                }
                if side_of_cell(x, y) != side_of_cell(other_x as usize, other_y as usize) {
                    assert!(walls.bars_step(cell(x as u8, y as u8), dx, dy), "{case_name}: ({x}, {y}) by ({dx}, {dy})");
                }
            }
        }
    }
}

/// A wall of one way: a drop along a column is stepped down by waves
/// and by A* alike and not back up, the diagonals over it as the
/// straight step; and on walls drawn each way of its own both find
/// the shortest way a search of every cell finds, there and back --
/// the two no longer one length.
#[test]
fn a_wall_of_one_way_bars_the_step_back_alone() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    // High ground west of column 8: no step west out of column 8, all the way down.
    let mut west_walls = [0; AREA_SIDE];
    west_walls.iter_mut().for_each(|west_row| *west_row = 1 << 8);
    let drop = Walls::one_way([0; AREA_SIDE], west_walls, [0; AREA_SIDE], [0; AREA_SIDE]);
    let (above_cell, below_cell) = (cell(6, 5), cell(10, 5));
    let goals_of = |goal_cell: Cell| {
        let mut goal_rows: Rows = [0; AREA_SIDE];
        goal_rows[goal_cell.y as usize] = 1 << goal_cell.x;
        goal_rows
    };
    assert_eq!(a_star(&EVERY_CELL, &drop, above_cell, below_cell).map(|path| path.steps), Some(4), "straight down it");
    assert_eq!(step_towards(&EVERY_CELL, &drop, &goals_of(below_cell), above_cell, 0).map(|path| path.steps), Some(4));
    assert_eq!(a_star(&EVERY_CELL, &drop, below_cell, above_cell), None, "and not back up");
    assert_eq!(step_towards(&EVERY_CELL, &drop, &goals_of(above_cell), below_cell, 0), None);
    for dy in [-1, 0, 1] {
        assert!(drop.bars_step(cell(8, 5), -1, dy) && !drop.bars_step(cell(7, 5), 1, dy), "by {dy} down");
    }

    let mut seeded_rng = RandomSource::new(seed);
    let (mut paths_found, mut unlike_lengths) = (0, 0);
    for _ in 0..200 {
        let walls = one_way_walls_drawn(&mut seeded_rng);
        let (from_cell, to_cell) = (cell(seeded_rng.draw_below(AREA_SIDE as u64) as u8, seeded_rng.draw_below(AREA_SIDE as u64) as u8), cell(seeded_rng.draw_below(AREA_SIDE as u64) as u8, seeded_rng.draw_below(AREA_SIDE as u64) as u8));
        if from_cell == to_cell {
            continue;
        }
        let (there_steps, back_steps) = (searched_steps(&EVERY_CELL, &walls, from_cell, to_cell), searched_steps(&EVERY_CELL, &walls, to_cell, from_cell));
        assert_eq!(a_star(&EVERY_CELL, &walls, from_cell, to_cell).map(|path| path.steps), there_steps, "{from_cell:?} to {to_cell:?}");
        assert_eq!(a_star(&EVERY_CELL, &walls, to_cell, from_cell).map(|path| path.steps), back_steps, "{to_cell:?} to {from_cell:?}");
        assert_eq!(step_towards(&EVERY_CELL, &walls, &goals_of(to_cell), from_cell, seeded_rng.next_draw()).map(|path| path.steps), there_steps, "waves, {from_cell:?} to {to_cell:?}");
        // The first step of either is one no wall bars.
        if let Some(path) = a_star(&EVERY_CELL, &walls, from_cell, to_cell) {
            assert!(!walls.bars_step(from_cell, path.first_cell.x as i8 - from_cell.x as i8, path.first_cell.y as i8 - from_cell.y as i8));
        }
        (paths_found, unlike_lengths) = (paths_found + there_steps.is_some() as u32, unlike_lengths + (there_steps != back_steps) as u32);
    }
    assert!(paths_found > 100 && unlike_lengths > 0, "{paths_found} ways found, {unlike_lengths} not the length of the way back");
}
