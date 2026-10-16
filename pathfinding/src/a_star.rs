//! A*: the shortest path from one cell of the area to another.

use super::{CELLS, Cell, Path, Rows, AREA_SIDE, rows_hold, steps_apart};
use super::walls::Walls;

/// The eight neighbours, as steps `(dx, dy)`.
const STEPS: [(i8, i8); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Room in a search's queue: a cell is queued again each time a shorter
/// way to it is found, from one neighbour after another at most.
const QUEUE_ROOM: usize = CELLS * STEPS.len();

/// A search's queue: the cells to look at next, the one on the shortest
/// path guessed first -- a binary heap in an array.
struct SearchQueue {
    /// The entries: a path's guessed length, then its steps so far
    /// counted down -- of two guessed equal, the further along first --
    /// then the cell, packed so the least is the next.
    heap_entries: [u32; QUEUE_ROOM],
    /// How many there are.
    len: usize,
}

impl SearchQueue {
    /// Queues the cell at `area_index`, `steps` steps along a path guessed
    /// `guessed_length` steps long.
    fn push(&mut self, guessed_length: u8, steps: u8, area_index: usize) {
        debug_assert!(self.len < QUEUE_ROOM, "a cell queued more often than it has neighbours");
        let mut heap_index = self.len;
        self.heap_entries[heap_index] = (guessed_length as u32) << 16 | ((u8::MAX - steps) as u32) << 8 | area_index as u32;
        self.len += 1;
        while heap_index > 0 && self.heap_entries[heap_index] < self.heap_entries[(heap_index - 1) / 2] {
            self.heap_entries.swap(heap_index, (heap_index - 1) / 2);
            heap_index = (heap_index - 1) / 2;
        }
    }

    /// Takes the next cell out: its index in the area, and the steps to
    /// it when it was queued.
    fn pop(&mut self) -> Option<(usize, u8)> {
        if self.len == 0 {
            return None;
        }
        let next_entry = self.heap_entries[0];
        self.len -= 1;
        self.heap_entries[0] = self.heap_entries[self.len];
        let mut heap_index = 0;
        loop {
            let (left_child, right_child) = (2 * heap_index + 1, 2 * heap_index + 2);
            let mut least_index = heap_index;
            if left_child < self.len && self.heap_entries[left_child] < self.heap_entries[least_index] {
                least_index = left_child;
            }
            if right_child < self.len && self.heap_entries[right_child] < self.heap_entries[least_index] {
                least_index = right_child;
            }
            if least_index == heap_index {
                break;
            }
            self.heap_entries.swap(heap_index, least_index);
            heap_index = least_index;
        }
        Some(((next_entry & 0xff) as usize, u8::MAX - (next_entry >> 8 & 0xff) as u8))
    }
}

/// The shortest path from `from_cell` to `to_cell` over the cells of `passable`,
/// through no wall of `walls`: its first step and its length. `None`
/// if there is no way, or if they are one cell (`docs/pathfinding.md`,
/// "A*").
pub fn a_star(passable: &Rows, walls: &Walls, from_cell: Cell, to_cell: Cell) -> Option<Path> {
    if from_cell == to_cell {
        return None;
    }
    // Steps from `to_cell` to each cell, the shortest found so far.
    let mut steps = [u8::MAX; CELLS];
    // The cell each was reached from: one step nearer `to_cell`.
    let mut reached_from = [0u8; CELLS];
    // The cells whose shortest path is settled.
    let mut settled_cells: Rows = [0; AREA_SIDE];
    let mut search_queue = SearchQueue { heap_entries: [0; QUEUE_ROOM], len: 0 };
    steps[to_cell.area_index()] = 0;
    search_queue.push(steps_apart(to_cell, from_cell), 0, to_cell.area_index());
    while let Some((popped_index, queued_at)) = search_queue.pop() {
        let cell = Cell::from_index(popped_index);
        // Queued again since by a shorter way, and looked at then.
        if rows_hold(&settled_cells, cell) || queued_at > steps[popped_index] {
            continue;
        }
        if cell == from_cell {
            return Some(Path { first_cell: Cell::from_index(reached_from[popped_index] as usize), steps: steps[popped_index] });
        }
        settled_cells[cell.y as usize] |= 1 << cell.x;
        let further_steps = steps[popped_index] + 1;
        for (dx, dy) in STEPS {
            let (x, y) = (cell.x as i8 + dx, cell.y as i8 + dy);
            if x < 0 || y < 0 || x >= AREA_SIDE as i8 || y >= AREA_SIDE as i8 {
                continue;
            }
            let neighbour_cell = Cell { x: x as u8, y: y as u8 };
            if neighbour_cell != from_cell && !rows_hold(passable, neighbour_cell) || walls.bars_step(neighbour_cell, -dx, -dy) || further_steps >= steps[neighbour_cell.area_index()] {
                continue;
            }
            steps[neighbour_cell.area_index()] = further_steps;
            reached_from[neighbour_cell.area_index()] = popped_index as u8;
            search_queue.push(further_steps + steps_apart(neighbour_cell, from_cell), further_steps, neighbour_cell.area_index());
        }
    }
    None
}
