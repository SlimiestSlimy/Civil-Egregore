//! A*: the shortest path from one cell of the area to another.

use super::{CELLS, Cell, Path, Rows, SIDE, holds, steps_apart};
use super::walls::Walls;

/// The eight neighbours, as steps `(dx, dy)`.
const STEPS: [(i8, i8); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Room in a search's queue: a cell is queued again each time a shorter
/// way to it is found, from one neighbour after another at most.
const QUEUE: usize = CELLS * STEPS.len();

/// A search's queue: the cells to look at next, the one on the shortest
/// path guessed first -- a binary heap in an array.
struct Queue {
    /// The entries: a path's guessed length, then its steps so far
    /// counted down -- of two guessed equal, the further along first --
    /// then the cell, packed so the least is the next.
    entries: [u32; QUEUE],
    /// How many there are.
    len: usize,
}

impl Queue {
    /// Queues the cell at `index`, `steps` steps along a path guessed
    /// `guessed` steps long.
    fn push(&mut self, guessed: u8, steps: u8, index: usize) {
        debug_assert!(self.len < QUEUE, "a cell queued more often than it has neighbours");
        let mut at = self.len;
        self.entries[at] = (guessed as u32) << 16 | ((u8::MAX - steps) as u32) << 8 | index as u32;
        self.len += 1;
        while at > 0 && self.entries[at] < self.entries[(at - 1) / 2] {
            self.entries.swap(at, (at - 1) / 2);
            at = (at - 1) / 2;
        }
    }

    /// Takes the next cell out: its index in the area, and the steps to
    /// it when it was queued.
    fn pop(&mut self) -> Option<(usize, u8)> {
        if self.len == 0 {
            return None;
        }
        let next = self.entries[0];
        self.len -= 1;
        self.entries[0] = self.entries[self.len];
        let mut at = 0;
        loop {
            let (left, right) = (2 * at + 1, 2 * at + 2);
            let mut least = at;
            if left < self.len && self.entries[left] < self.entries[least] {
                least = left;
            }
            if right < self.len && self.entries[right] < self.entries[least] {
                least = right;
            }
            if least == at {
                break;
            }
            self.entries.swap(at, least);
            at = least;
        }
        Some(((next & 0xff) as usize, u8::MAX - (next >> 8 & 0xff) as u8))
    }
}

/// The shortest path from `from` to `to` over the cells of `passable`,
/// through no wall of `walls`: its first step and its length. `None`
/// if there is no way, or if they are one cell (`docs/pathfinding.md`,
/// "A*").
pub fn a_star(passable: &Rows, walls: &Walls, from: Cell, to: Cell) -> Option<Path> {
    if from == to {
        return None;
    }
    // Steps from `to` to each cell, the shortest found so far.
    let mut steps = [u8::MAX; CELLS];
    // The cell each was reached from: one step nearer `to`.
    let mut towards = [0u8; CELLS];
    // The cells whose shortest path is settled.
    let mut settled: Rows = [0; SIDE];
    let mut queue = Queue { entries: [0; QUEUE], len: 0 };
    steps[to.index()] = 0;
    queue.push(steps_apart(to, from), 0, to.index());
    while let Some((index, queued_at)) = queue.pop() {
        let cell = Cell::at(index);
        // Queued again since by a shorter way, and looked at then.
        if holds(&settled, cell) || queued_at > steps[index] {
            continue;
        }
        if cell == from {
            return Some(Path { first: Cell::at(towards[index] as usize), steps: steps[index] });
        }
        settled[cell.y as usize] |= 1 << cell.x;
        let further = steps[index] + 1;
        for (dx, dy) in STEPS {
            let (x, y) = (cell.x as i8 + dx, cell.y as i8 + dy);
            if x < 0 || y < 0 || x >= SIDE as i8 || y >= SIDE as i8 {
                continue;
            }
            let neighbour = Cell { x: x as u8, y: y as u8 };
            if neighbour != from && !holds(passable, neighbour) || walls.bars_step(cell, dx, dy) || further >= steps[neighbour.index()] {
                continue;
            }
            steps[neighbour.index()] = further;
            towards[neighbour.index()] = index as u8;
            queue.push(further + steps_apart(neighbour, from), further, neighbour.index());
        }
    }
    None
}
