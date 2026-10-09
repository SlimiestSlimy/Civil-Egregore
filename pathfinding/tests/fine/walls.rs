//! Walls between cells: barring a step both ways, one thick in any
//! direction, never crossed by waves, by A* or by a walker.
//!
//! `cargo test`

use crate::tests::*;
use utilities::rng::Rng;
use pathfinding::{a_star, holds, step_towards, Cell, Rows, Walls, Wave, SIDE};

/// A wall bars the step between two cells, both ways, whatever the cells
/// are: a wall across the area with one gap is gone round by waves and
/// by A* alike, and one with none is not crossed; a diagonal step is
/// barred unless both ways round it are open.
#[test]
fn walls_bar_steps_between_cells() {
    // A wall under row 7, all the way across.
    let mut south = [0; SIDE];
    south[7] = u16::MAX;
    let walls = Walls::new([0; SIDE], south);
    let (from, to) = (cell(3, 5), cell(3, 10));
    let mut goals: Rows = [0; SIDE];
    goals[to.y as usize] = 1 << to.x;
    assert_eq!(a_star(&ALL, &walls, from, to), None, "no way through");
    assert_eq!(a_star(&ALL, &walls, to, from), None, "nor back");
    assert_eq!(step_towards(&ALL, &walls, &goals, from, 0), None);

    // A gap at column 12: the straight step down alone, the diagonals through it each walled on a way round.
    south[7] &= !(1 << 12);
    let walls = Walls::new([0; SIDE], south);
    let path = a_star(&ALL, &walls, from, to).expect("through the gap");
    assert_eq!(path.steps, 9 + 1 + 9, "nine across to the gap's column, down through it, and nine back");
    let wave = step_towards(&ALL, &walls, &goals, from, 0).expect("through the gap");
    assert_eq!(wave.steps, path.steps, "waves and A* agree");
    // Walked a step at a time, it gets there in as many steps, never through the wall.
    let (mut at, mut taken) = (from, 0);
    while at != to {
        let next = step_towards(&ALL, &walls, &goals, at, taken).expect("still a way").first;
        assert!(!walls.bars_step(at, next.x as i8 - at.x as i8, next.y as i8 - at.y as i8), "{at:?} to {next:?} through a wall");
        (at, taken) = (next, taken + 1);
    }
    assert_eq!(taken, path.steps as u64);

    // One wall, east of (4, 4): it bars the step across it and every diagonal with it on a way round, both ways.
    let mut east = [0; SIDE];
    east[4] = 1 << 4;
    let corner = Walls::new(east, [0; SIDE]);
    for (from, (dx, dy)) in [((4, 4), (1, 0)), ((4, 4), (1, 1)), ((5, 4), (-1, 1)), ((4, 3), (1, 1)), ((5, 3), (-1, 1)), ((4, 4), (1, -1)), ((5, 5), (-1, -1))] {
        assert!(corner.bars_step(cell(from.0, from.1), dx, dy), "{from:?} by ({dx}, {dy})");
    }
    for (from, (dx, dy)) in [((4, 4), (0, 1)), ((4, 4), (0, -1)), ((4, 4), (-1, 1)), ((5, 4), (1, 1)), ((5, 4), (0, 1))] {
        assert!(!corner.bars_step(cell(from.0, from.1), dx, dy), "{from:?} by ({dx}, {dy})");
    }
    assert_eq!(a_star(&ALL, &corner, cell(4, 4), cell(5, 4)).map(|path| path.steps), Some(3), "round the wall's end, the diagonals barred");
}

/// The walls between cells of different heights, as the terrain puts
/// them: east and south of a cell more than one apart from its neighbour.
fn walls_of(height: impl Fn(usize, usize) -> u8) -> Walls {
    let (mut east, mut south) = ([0; SIDE], [0; SIDE]);
    for y in 0..SIDE {
        for x in 0..SIDE {
            if x + 1 < SIDE && height(x, y).abs_diff(height(x + 1, y)) > 1 {
                east[y] |= 1 << x;
            }
            if y + 1 < SIDE && height(x, y).abs_diff(height(x, y + 1)) > 1 {
                south[y] |= 1 << x;
            }
        }
    }
    Walls::new(east, south)
}

/// The cells reached from `start` by waves over every cell, through no
/// wall.
fn reached_from(start: Cell, walls: &Walls) -> Rows {
    let mut goals: Rows = [0; SIDE];
    goals[start.y as usize] = 1 << start.x;
    let mut wave = Wave::from(&goals);
    while wave.advance(&ALL, walls) {}
    *wave.reached()
}

/// A wall one thick, in any direction -- across, down, or diagonal, as
/// a cliff's edge or as a ridge of single cells -- is never crossed: by
/// waves, by A*, or by a walker stepping towards the far side. A
/// diagonal step across it always has a wall on one of its two ways
/// round, so it is barred.
#[test]
fn walls_one_thick_in_any_direction_are_never_crossed() {
    // Each case: the heights, and which side of the wall a cell is on (`None` on the ridge itself).
    type Height = fn(usize, usize) -> u8;
    type Side = fn(usize, usize) -> Option<bool>;
    let cases: [(&str, Height, Side); 6] = [
        ("a cliff down the middle", |x, _| if x < 8 { 9 } else { 0 }, |x, _| Some(x < 8)),
        ("a cliff across the middle", |_, y| if y < 8 { 9 } else { 0 }, |_, y| Some(y < 8)),
        ("a cliff along the diagonal", |x, y| if x > y { 9 } else { 0 }, |x, y| Some(x > y)),
        ("a cliff along the other diagonal", |x, y| if x + y < SIDE { 9 } else { 0 }, |x, y| Some(x + y < SIDE)),
        ("a ridge one cell thick along the diagonal", |x, y| if x == y { 9 } else { 0 }, |x, y| (x != y).then_some(x > y)),
        ("a ridge one cell thick along the other diagonal", |x, y| if x + y == SIDE - 1 { 9 } else { 0 }, |x, y| (x + y != SIDE - 1).then_some(x + y < SIDE - 1)),
    ];
    for (name, height, side) in cases {
        let walls = walls_of(height);
        let cells = || (0..SIDE).flat_map(|y| (0..SIDE).map(move |x| (x, y)));
        // Every cell of a side reaches every other of its side, and none of the other's, nor the ridge.
        for (x, y) in cells() {
            let Some(here) = side(x, y) else {
                continue;
            };
            let reached = reached_from(cell(x as u8, y as u8), &walls);
            for (other_x, other_y) in cells() {
                let got_there = holds(&reached, cell(other_x as u8, other_y as u8));
                assert_eq!(got_there, side(other_x, other_y) == Some(here), "{name}: ({x}, {y}) to ({other_x}, {other_y})");
            }
        }
        // A* and a walker find no way across, from either side.
        let mut rng = Rng::new(name.len() as u64);
        for _ in 0..200 {
            let draw = |rng: &mut Rng| cell((rng.draw() % SIDE as u64) as u8, (rng.draw() % SIDE as u64) as u8);
            let (from, to) = (draw(&mut rng), draw(&mut rng));
            let (Some(from_side), Some(to_side)) = (side(from.x as usize, from.y as usize), side(to.x as usize, to.y as usize)) else {
                continue;
            };
            if from_side == to_side || from == to {
                continue;
            }
            assert_eq!(a_star(&ALL, &walls, from, to), None, "{name}: A* from {from:?} to {to:?}");
            let mut goals: Rows = [0; SIDE];
            goals[to.y as usize] = 1 << to.x;
            assert_eq!(step_towards(&ALL, &walls, &goals, from, rng.draw()), None, "{name}: a walker from {from:?} to {to:?}");
        }
        // No step between neighbours on two sides is open, diagonals included.
        for (x, y) in cells() {
            for (dx, dy) in [(1i8, 0i8), (0, 1), (1, 1), (-1, 1)] {
                let (other_x, other_y) = (x as i8 + dx, y as i8 + dy);
                if other_x < 0 || other_x >= SIDE as i8 || other_y >= SIDE as i8 {
                    continue;
                }
                if side(x, y) != side(other_x as usize, other_y as usize) {
                    assert!(walls.bars_step(cell(x as u8, y as u8), dx, dy), "{name}: ({x}, {y}) by ({dx}, {dy})");
                }
            }
        }
    }
}
