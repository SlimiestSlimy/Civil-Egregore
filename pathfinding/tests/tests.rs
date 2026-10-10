//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use pathfinding::{holds, Cell, Rows, Walls, SIDE};


/// No walls.
pub const OPEN: Walls = Walls::new([0; SIDE], [0; SIDE]);

/// Every cell.
pub const ALL: Rows = [u16::MAX; SIDE];

/// The cell `(x, y)`.
pub fn cell(x: u8, y: u8) -> Cell {
    Cell { x, y }
}

/// Walls drawn: about one step in eight barred each way.
pub fn walls_drawn(random: &mut utilities::rng::Rng) -> Walls {
    let mut rows = || std::array::from_fn(|_| (random.draw() & random.draw() >> 16 & random.draw() >> 32) as u16);
    Walls::new(rows(), rows())
}

/// The steps of the shortest path from `from` to `to` over `passable`
/// and through no wall of `walls`, by a search of every cell, ring
/// after ring: what A* and the waves are judged by.
pub fn searched(passable: &Rows, walls: &Walls, from: Cell, to: Cell) -> Option<u8> {
    let mut reached = vec![vec![false; SIDE]; SIDE];
    reached[from.y as usize][from.x as usize] = true;
    let mut ring = vec![from];
    for steps in 1..=u8::MAX {
        let mut next = Vec::new();
        for at in ring {
            for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                let (x, y) = (at.x as i32 + dx, at.y as i32 + dy);
                if x < 0 || y < 0 || x >= SIDE as i32 || y >= SIDE as i32 || reached[y as usize][x as usize] || walls.bars_step(at, dx as i8, dy as i8) {
                    continue;
                }
                let neighbour = cell(x as u8, y as u8);
                if neighbour == to {
                    return Some(steps);
                }
                if holds(passable, neighbour) {
                    reached[y as usize][x as usize] = true;
                    next.push(neighbour);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        ring = next;
    }
    None
}
