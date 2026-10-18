//! Civil Egregore's pathfinding: over an **area**, 16x16 cells kept as
//! masks -- a row a `u16`, cell `(x, y)` at bit `x` of row `y` -- which
//! is all it knows of the world. Waves from many goals ([`Wave`],
//! [`step_towards`]), the nearest cell ([`nearest_goal`]), and A* between two
//! cells ([`a_star`]); nothing here allocates.
//!
//! The design: `docs/pathfinding.md`; function by function:
//! `docs/pathfinding_reference.md`.


mod a_star;
// The search in Morton order: no rule walks by it yet, only its tests.
#[cfg_attr(not(test), allow(dead_code))]
pub mod morton;
mod walls;

pub use a_star::a_star;
pub use walls::Walls;

use walls::{spread_forwards, spread_back};

/// Cells along an area's side.
pub const AREA_SIDE: usize = 16;

/// Cells in an area.
const CELLS: usize = AREA_SIDE * AREA_SIDE;

/// An area's cells of one kind: a row a word, cell `(x, y)` at bit `x`
/// of row `y`.
pub type Rows = [u16; AREA_SIDE];

/// A cell of an area: `(x, y)` from its top left, each under [`AREA_SIDE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Its column.
    pub x: u8,
    /// Its row.
    pub y: u8,
}

impl Cell {
    /// Its index in the area, row by row.
    const fn area_index(self) -> usize {
        self.y as usize * AREA_SIDE + self.x as usize
    }

    /// The cell at `area_index`, row by row.
    const fn from_index(area_index: usize) -> Self {
        Self { x: (area_index % AREA_SIDE) as u8, y: (area_index / AREA_SIDE) as u8 }
    }
}

/// A path found: where it goes first, and how long it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    /// The first cell on it after the one it starts from.
    pub first_cell: Cell,
    /// Its steps, start to end.
    pub steps: u8,
}

/// Whether `area_rows` holds `cell`.
pub const fn rows_hold(area_rows: &Rows, cell: Cell) -> bool {
    area_rows[cell.y as usize] >> cell.x & 1 == 1
}

/// The steps from `from_cell` to `to_cell` with nothing in the way: the greater of
/// their distances across and down, a step reaching any neighbour.
pub const fn steps_apart(from_cell: Cell, to_cell: Cell) -> u8 {
    let (cells_across, cells_down) = (from_cell.x.abs_diff(to_cell.x), from_cell.y.abs_diff(to_cell.y));
    if cells_across > cells_down { cells_across } else { cells_down }
}

/// The cell of `goal_rows` nearest `from_cell` in steps, nothing in the way
/// counted, other than `from_cell` itself: of those equally near, the
/// `pick_number`-th, round and round -- a random number, so walkers do not all
/// lean one way. `None` if `goal_rows` holds no other cell.
#[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
pub fn nearest_goal(goal_rows: &Rows, from_cell: Cell, pick_number: u64) -> Option<Cell> {
    let (mut least_apart, mut equally_near) = (u8::MAX, 0u64);
    for_each(goal_rows, from_cell, |cell| {
        let cell_distance = steps_apart(from_cell, cell);
        if cell_distance < least_apart {
            (least_apart, equally_near) = (cell_distance, 0);
        }
        equally_near += (cell_distance == least_apart) as u64;
    });
    if equally_near == 0 {
        return None;
    }
    let (mut picks_left, mut found_cell) = (pick_number % equally_near, None);
    for_each(goal_rows, from_cell, |cell| {
        if steps_apart(from_cell, cell) == least_apart {
            if picks_left == 0 && found_cell.is_none() {
                found_cell = Some(cell);
            }
            picks_left = picks_left.wrapping_sub(1);
        }
    });
    found_cell
}

/// `each_cell` on every cell of `area_rows` but `skipped_cell`, row by row.
#[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
fn for_each(area_rows: &Rows, skipped_cell: Cell, mut each_cell: impl FnMut(Cell)) {
    for (y, &row_bits) in area_rows.iter().enumerate() {
        let mut remaining_bits = if y == skipped_cell.y as usize { row_bits & !(1 << skipped_cell.x) } else { row_bits };
        while remaining_bits != 0 {
            each_cell(Cell { x: remaining_bits.trailing_zeros() as u8, y: y as u8 });
            remaining_bits &= remaining_bits - 1;
        }
    }
}

/// A search spreading from every goal at once: the cells within so many
/// steps of a goal, over the cells that may be walked on, a step further
/// each [`Wave::spread_one_step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wave {
    /// The cells reached so far.
    reached_rows: Rows,
}

impl Wave {
    /// A wave yet to spread: `goal_rows` alone reached.
    pub const fn from_goals(goal_rows: &Rows) -> Self {
        Self { reached_rows: *goal_rows }
    }

    /// The cells reached so far.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub const fn reached_rows(&self) -> &Rows {
        &self.reached_rows
    }

    /// One step: every passable neighbour of a reached cell reached,
    /// no wall before its step onto that cell -- the way a walker
    /// goes, towards the goals. Whether any was -- if not, the wave has
    /// gone as far as it can.
    pub fn spread_one_step(&mut self, passable: &Rows, walls: &Walls) -> bool {
        let spread_rows = spread_back(&self.reached_rows, walls);
        let mut grown_bits = 0;
        for ((reached_row, &spread_row), &passable) in self.reached_rows.iter_mut().zip(&spread_rows).zip(passable) {
            let new_cells = spread_row & passable & !*reached_row;
            *reached_row |= new_cells;
            grown_bits |= new_cells;
        }
        grown_bits != 0
    }
}

/// A walker on `from_cell`'s next step to the nearest cell of `goal_rows` over
/// the cells of `passable`, through no wall of `walls`, and how many
/// steps away that goal is; `None` if none can be walked to
/// (`docs/pathfinding.md`, "Waves").
pub fn step_towards(passable: &Rows, walls: &Walls, goal_rows: &Rows, from_cell: Cell, pick_number: u64) -> Option<Path> {
    let mut beside_rows: Rows = [0; AREA_SIDE];
    beside_rows[from_cell.y as usize] = 1 << from_cell.x;
    beside_rows = spread_forwards(&beside_rows, walls);
    beside_rows[from_cell.y as usize] &= !(1 << from_cell.x);
    let mut wave = Wave::from_goals(goal_rows);
    wave.reached_rows[from_cell.y as usize] &= !(1 << from_cell.x);
    let mut steps = 1;
    loop {
        let (mut arrived_rows, mut count): (Rows, u32) = ([0; AREA_SIDE], 0);
        for ((arrived_row, &beside_row), &reached_row) in arrived_rows.iter_mut().zip(&beside_rows).zip(&wave.reached_rows) {
            *arrived_row = beside_row & reached_row;
            count += arrived_row.count_ones();
        }
        if count > 0 {
            let mut picks_left = pick_number % count as u64;
            for (y, &row_bits) in arrived_rows.iter().enumerate() {
                let mut remaining_bits = row_bits;
                while remaining_bits != 0 {
                    if picks_left == 0 {
                        return Some(Path { first_cell: Cell { x: remaining_bits.trailing_zeros() as u8, y: y as u8 }, steps });
                    }
                    picks_left -= 1;
                    remaining_bits &= remaining_bits - 1;
                }
            }
        }
        if !wave.spread_one_step(passable, walls) {
            return None;
        }
        steps += 1;
    }
}
