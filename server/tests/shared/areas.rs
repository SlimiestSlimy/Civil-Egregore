//! Areas, walls and searches as the tiers' tests share them: in a
//! folder of its own, so no test program of its own, a module of each
//! tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use server::pathfinding::{rows_hold, Cell, Rows, Walls, AREA_SIDE};


/// No walls.
pub const NO_WALLS: Walls = Walls::new([0; AREA_SIDE], [0; AREA_SIDE]);

/// Every cell.
pub const EVERY_CELL: Rows = [u16::MAX; AREA_SIDE];

/// The cell `(x, y)`.
pub fn cell(x: u8, y: u8) -> Cell {
    Cell { x, y }
}

/// Walls drawn: about one step in eight barred each way.
pub fn walls_drawn(seeded_rng: &mut utilities::rng::RandomSource) -> Walls {
    let mut drawn_rows = || std::array::from_fn(|_| (seeded_rng.next_draw() & seeded_rng.next_draw() >> 16 & seeded_rng.next_draw() >> 32) as u16);
    Walls::new(drawn_rows(), drawn_rows())
}

/// Walls drawn each way of its own: about one step in eight barred
/// out of a cell north, west, south, east, whatever the step back is.
pub fn one_way_walls_drawn(seeded_rng: &mut utilities::rng::RandomSource) -> Walls {
    let mut drawn_rows = || std::array::from_fn(|_| (seeded_rng.next_draw() & seeded_rng.next_draw() >> 16 & seeded_rng.next_draw() >> 32) as u16);
    Walls::one_way(drawn_rows(), drawn_rows(), drawn_rows(), drawn_rows())
}

/// The steps of the shortest path from `from_cell` to `to_cell` over `passable`
/// and through no wall of `walls`, by a search of every cell, ring
/// after ring: what A* and the waves are judged by.
pub fn searched_steps(passable: &Rows, walls: &Walls, from_cell: Cell, to_cell: Cell) -> Option<u8> {
    let mut reached_cells = vec![vec![false; AREA_SIDE]; AREA_SIDE];
    reached_cells[from_cell.y as usize][from_cell.x as usize] = true;
    let mut ring_cells = vec![from_cell];
    for steps in 1..=u8::MAX {
        let mut next_ring = Vec::new();
        for ring_cell in ring_cells {
            for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                let (x, y) = (ring_cell.x as i32 + dx, ring_cell.y as i32 + dy);
                if x < 0 || y < 0 || x >= AREA_SIDE as i32 || y >= AREA_SIDE as i32 || reached_cells[y as usize][x as usize] || walls.bars_step(ring_cell, dx as i8, dy as i8) {
                    continue;
                }
                let neighbour_cell = cell(x as u8, y as u8);
                if neighbour_cell == to_cell {
                    return Some(steps);
                }
                if rows_hold(passable, neighbour_cell) {
                    reached_cells[y as usize][x as usize] = true;
                    next_ring.push(neighbour_cell);
                }
            }
        }
        if next_ring.is_empty() {
            return None;
        }
        ring_cells = next_ring;
    }
    None
}
