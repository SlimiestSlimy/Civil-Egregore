//! Paths over open ground and round what is in the way: A*'s the
//! shortest, the nearest goal picked, waves spreading a cell a step.
//!
//! `cargo test`

use crate::tests::*;
use utilities::rng::Rng;
use pathfinding::{a_star, holds, nearest, step_towards, steps_apart, Cell, Path, Rows, Wave, SIDE};

/// A cell of the area, drawn.
fn cell_drawn(random: &mut Rng) -> Cell {
    cell(random.below(SIDE as u64) as u8, random.below(SIDE as u64) as u8)
}

/// With nothing in the way a path is as long as the cells are apart,
/// and its first step is a neighbour one step nearer.
#[test]
fn paths_over_open_ground_are_straight() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut pairs = vec![(cell(8, 8), cell(15, 8)), (cell(8, 8), cell(0, 0)), (cell(0, 15), cell(15, 0)), (cell(8, 8), cell(9, 9))];
    pairs.extend((0..300).map(|_| (cell_drawn(&mut random), cell_drawn(&mut random))).filter(|(from, to)| from != to));
    for (from, to) in pairs {
        let path = a_star(&ALL, &OPEN, from, to).expect("a way");
        assert_eq!(path.steps, steps_apart(from, to));
        assert_eq!(steps_apart(from, path.first), 1);
        assert_eq!(steps_apart(path.first, to), path.steps - 1);
    }
    assert_eq!(a_star(&ALL, &OPEN, cell(4, 4), cell(4, 4)), None, "nowhere to go");
}

/// A wall with one gap is walked round through the gap; with none,
/// there is no way.
#[test]
fn paths_go_round_what_is_in_the_way() {
    let mut passable = ALL;
    // A wall down column 10, open at row 15 only.
    for row in &mut passable[..15] {
        *row &= !(1 << 10);
    }
    let (from, to) = (cell(8, 2), cell(12, 2));
    let path = a_star(&passable, &OPEN, from, to).expect("through the gap");
    assert_eq!(Some(path.steps), searched(&passable, &OPEN, from, to));
    assert_eq!(path.steps, 13 + 13, "down to the gap and back up");
    passable[15] &= !(1 << 10);
    assert_eq!(a_star(&passable, &OPEN, from, to), None, "walled off");
    assert_eq!(a_star(&passable, &OPEN, from, cell(10, 2)).map(|path| path.steps), Some(2), "the end need not be passable");
    assert_eq!(a_star(&[0; SIDE], &OPEN, cell(1, 1), cell(2, 2)).map(|path| path.steps), Some(1), "nor the start");
}

/// Walking a path a first step at a time arrives in the steps it said,
/// over passable cells only, and A*'s path is as short as a search of
/// every cell finds -- on areas of obstacles drawn at random, two in
/// three with walls drawn between their cells too.
#[test]
fn paths_are_the_shortest_and_can_be_walked() {
    let mut random = Rng::new(utilities::seed::counted());
    let (mut found, mut none) = (0, 0);
    for case in 0..3000 {
        // About a third of the cells in the way.
        let passable: Rows = std::array::from_fn(|_| (random.draw() | random.draw() >> 16 & random.draw()) as u16);
        let walls = if case % 3 == 0 { OPEN } else { walls_drawn(&mut random) };
        let (from, to) = (cell_drawn(&mut random), cell_drawn(&mut random));
        if from == to {
            continue;
        }
        let path = a_star(&passable, &walls, from, to);
        assert_eq!(path.map(|path| path.steps), searched(&passable, &walls, from, to), "{from:?} to {to:?}");
        let Some(Path { steps, .. }) = path else {
            none += 1;
            continue;
        };
        found += 1;
        let (mut at, mut walked) = (from, 0);
        while at != to {
            let next = a_star(&passable, &walls, at, to).expect("still a way").first;
            assert_eq!(steps_apart(at, next), 1);
            assert!(next == to || holds(&passable, next), "walked onto what is in the way");
            assert!(!walls.bars_step(at, next.x as i8 - at.x as i8, next.y as i8 - at.y as i8), "walked through a wall");
            (at, walked) = (next, walked + 1);
        }
        assert_eq!(walked, steps);
    }
    assert!(found > 500 && none > 20, "{found} found, {none} with no way");
}

/// The nearest cell of a mask is as near as any, never the cell asked
/// from, and those equally near are each picked in turn.
#[test]
fn the_nearest_is_picked_among_the_equally_near() {
    let from = cell(8, 8);
    assert_eq!(nearest(&[0; SIDE], from, 0), None);
    let mut goals: Rows = [0; SIDE];
    goals[8] |= 1 << 8;
    assert_eq!(nearest(&goals, from, 0), None, "its own cell is nowhere to go");
    goals[8] |= 1 << 11;
    goals[5] |= 1 << 8;
    goals[11] |= 1 << 11;
    goals[0] |= 1;
    let mut picked: Vec<Cell> = (0..3).map(|pick| nearest(&goals, from, pick).expect("goals")).collect();
    assert_eq!(nearest(&goals, from, 3), Some(picked[0]), "round and round");
    picked.sort_by_key(|cell| (cell.y, cell.x));
    assert_eq!(picked, [cell(8, 5), cell(11, 8), cell(11, 11)], "the three three steps away, not the one eight away");
}

/// Among goals drawn, from a cell drawn: the nearest is as near as
/// any goal but the cell itself, and picks one after another go
/// through every goal that near, each once, and round again.
#[test]
fn the_nearest_of_goals_drawn_is_as_near_as_any() {
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..1000 {
        let mut goals: Rows = [0; SIDE];
        for _ in 0..random.below(12) {
            let goal = cell_drawn(&mut random);
            goals[goal.y as usize] |= 1 << goal.x;
        }
        let from = cell_drawn(&mut random);
        let others: Vec<Cell> = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).filter(|&goal| holds(&goals, goal) && goal != from).collect();
        let Some(least) = others.iter().map(|&goal| steps_apart(from, goal)).min() else {
            assert_eq!(nearest(&goals, from, random.draw()), None, "no goal but its own cell");
            continue;
        };
        let mut as_near: Vec<Cell> = others.into_iter().filter(|&goal| steps_apart(from, goal) == least).collect();
        let first = random.below(1 << 32);
        let mut picked: Vec<Cell> = (0..as_near.len() as u64).map(|pick| nearest(&goals, from, first + pick).expect("goals")).collect();
        assert_eq!(nearest(&goals, from, first + as_near.len() as u64), Some(picked[0]), "round and round");
        picked.sort_by_key(|cell| (cell.y, cell.x));
        as_near.sort_by_key(|cell| (cell.y, cell.x));
        assert_eq!(picked, as_near, "from {from:?}");
    }
}

/// A wave spreads a cell a step over open ground, a square about its
/// goal, and stops at what is in the way.
#[test]
fn waves_spread_a_cell_a_step() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut goals: Rows = [0; SIDE];
    // From the middle, from a corner, and from cells drawn.
    for goal in [cell(8, 8), cell(0, 15)].into_iter().chain((0..40).map(|_| cell_drawn(&mut random))) {
        goals = [0; SIDE];
        goals[goal.y as usize] = 1 << goal.x;
        let mut wave = Wave::from(&goals);
        let farthest = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).map(|other| steps_apart(goal, other)).max().expect("cells");
        for steps in 1..=farthest {
            assert!(wave.advance(&ALL, &OPEN), "from {goal:?}, step {steps}");
            for (x, y) in (0..16).flat_map(|y| (0..16).map(move |x| (x, y))) {
                assert_eq!(holds(wave.reached(), cell(x, y)), steps_apart(goal, cell(x, y)) <= steps, "from {goal:?}: ({x}, {y}) after {steps} steps");
            }
        }
        assert!(!wave.advance(&ALL, &OPEN), "from {goal:?}: every cell reached, nothing more to spread to");
    }
    let mut walled = Wave::from(&goals);
    assert!(!walled.advance(&[0; SIDE], &OPEN), "nowhere to spread");
    assert_eq!(walled.reached(), &goals);
}

/// A walker's step by waves is a first step of a shortest path to the
/// goal nearest over what may be walked on -- as far as a search of
/// every cell finds the nearest -- and walking those steps arrives.
#[test]
fn steps_by_waves_lead_to_the_nearest_goal() {
    let mut random = Rng::new(utilities::seed::counted());
    let (mut found, mut none) = (0, 0);
    for case in 0..3000 {
        let passable: Rows = std::array::from_fn(|_| (random.draw() | random.draw() >> 16 & random.draw()) as u16);
        let walls = if case % 3 == 0 { OPEN } else { walls_drawn(&mut random) };
        // A few goals, on cells that may be walked on or not.
        let mut goals: Rows = [0; SIDE];
        for _ in 0..1 + random.draw() % 4 {
            goals[random.draw() as usize % SIDE] |= 1 << (random.draw() % 16);
        }
        let from = cell_drawn(&mut random);
        // Where the walker starts is nowhere to go: no goal, so it is not walked back to.
        goals[from.y as usize] &= !(1 << from.x);
        // Goals need not be passable to be walked onto; the waves spread from them over what is.
        let mut over = passable;
        over.iter_mut().zip(&goals).for_each(|(row, goals)| *row |= goals);
        let every_goal = (0..16).flat_map(|y| (0..16).map(move |x| cell(x, y))).filter(|&goal| holds(&goals, goal));
        let expected = every_goal.filter_map(|goal| searched(&over, &walls, from, goal)).min();
        let step = step_towards(&over, &walls, &goals, from, random.draw());
        assert_eq!(step.map(|path| path.steps), expected, "from {from:?}");
        let Some(Path { steps, .. }) = step else {
            none += 1;
            continue;
        };
        found += 1;
        let (mut at, mut walked) = (from, 0);
        while !holds(&goals, at) {
            let next = step_towards(&over, &walls, &goals, at, random.draw()).expect("still a way").first;
            assert_eq!(steps_apart(at, next), 1);
            assert!(holds(&over, next), "walked onto what is in the way");
            assert!(!walls.bars_step(at, next.x as i8 - at.x as i8, next.y as i8 - at.y as i8), "walked through a wall");
            (at, walked) = (next, walked + 1);
            assert!(walked <= steps, "walked further than the path");
        }
        assert_eq!(walked, steps);
    }
    assert!(found > 500 && none > 20, "{found} found, {none} with no way");
}
