//! Civil Egregore's pathfinding: A* over an **area**, 16x16 cells kept as
//! masks -- a row a `u16`, cell `(x, y)` at bit `x` of row `y` -- which
//! is all it knows of the world. What the cells are, which may be walked
//! on and where the walker wants to go are for whoever calls it; an
//! entity's turn reads an area of the bitplanes around a cell
//! (`instructions::read::area::layer`) and hands it here.
//!
//! | what | what it does |
//! |---|---|
//! | [`Wave`] | a search spreading from every goal at once, a cell further each step, the whole area's cells in a few word operations |
//! | [`step_towards`] | a walker's next step to the nearest of many goals, by waves: what an entity asks each time it ticks |
//! | [`nearest`] | the cell of a mask nearest another, in steps, nothing in the way counted |
//! | [`a_star`] | the shortest path between two cells over the cells that may be walked on: its first step and its length |
//!
//! A walker takes a pathfinding step a tick of its own, and keeps no
//! route: each time it asks only where to step next, the world having
//! changed since it last asked. What that costs is waves: all of a
//! search's memory is the cells reached, 32 bytes, beside the 32 of the
//! cells that may be walked on -- one line of cache -- and a wave moves
//! every reached cell's front at once, a row a few shifts and ors.
//!
//! A step is to any of a cell's eight neighbours, each costing one: the
//! steps between two cells with nothing in the way are the greater of
//! their distances across and down ([`steps_apart`]), which is what A*
//! guesses the rest of a path by.
//!
//! Nothing here allocates: a search's working memory is a few arrays on
//! the stack, the size of the area. The design: `docs/pathfinding.md`;
//! function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod a_star;
pub mod diagnostics;
pub mod transient_data;
mod walls;

pub use a_star::a_star;
pub use walls::Walls;

use walls::spread;

/// Cells along an area's side.
pub const SIDE: usize = 16;

/// Cells in an area.
const CELLS: usize = SIDE * SIDE;

/// An area's cells of one kind: a row a word, cell `(x, y)` at bit `x`
/// of row `y`.
pub type Rows = [u16; SIDE];

/// A cell of an area: `(x, y)` from its top left, each under [`SIDE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Its column.
    pub x: u8,
    /// Its row.
    pub y: u8,
}

impl Cell {
    /// Its index in the area, row by row.
    const fn index(self) -> usize {
        self.y as usize * SIDE + self.x as usize
    }

    /// The cell at `index`, row by row.
    const fn at(index: usize) -> Self {
        Self { x: (index % SIDE) as u8, y: (index / SIDE) as u8 }
    }
}

/// A path found: where it goes first, and how long it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    /// The first cell on it after the one it starts from.
    pub first: Cell,
    /// Its steps, start to end.
    pub steps: u8,
}

/// Whether `rows` holds `cell`.
pub const fn holds(rows: &Rows, cell: Cell) -> bool {
    rows[cell.y as usize] >> cell.x & 1 == 1
}

/// The steps from `a` to `b` with nothing in the way: the greater of
/// their distances across and down, a step reaching any neighbour.
pub const fn steps_apart(a: Cell, b: Cell) -> u8 {
    let (across, down) = (a.x.abs_diff(b.x), a.y.abs_diff(b.y));
    if across > down { across } else { down }
}

/// The cell of `goals` nearest `from` in steps, nothing in the way
/// counted, other than `from` itself: of those equally near, the
/// `pick`-th, round and round -- a random number, so walkers do not all
/// lean one way. `None` if `goals` holds no other cell.
pub fn nearest(goals: &Rows, from: Cell, pick: u64) -> Option<Cell> {
    let (mut least, mut equally_near) = (u8::MAX, 0u64);
    for_each(goals, from, |cell| {
        let apart = steps_apart(from, cell);
        if apart < least {
            (least, equally_near) = (apart, 0);
        }
        equally_near += (apart == least) as u64;
    });
    if equally_near == 0 {
        return None;
    }
    let (mut left, mut found) = (pick % equally_near, None);
    for_each(goals, from, |cell| {
        if steps_apart(from, cell) == least {
            if left == 0 && found.is_none() {
                found = Some(cell);
            }
            left = left.wrapping_sub(1);
        }
    });
    found
}

/// `each` on every cell of `rows` but `from`, row by row.
fn for_each(rows: &Rows, from: Cell, mut each: impl FnMut(Cell)) {
    for (y, &row) in rows.iter().enumerate() {
        let mut bits = if y == from.y as usize { row & !(1 << from.x) } else { row };
        while bits != 0 {
            each(Cell { x: bits.trailing_zeros() as u8, y: y as u8 });
            bits &= bits - 1;
        }
    }
}

/// A search spreading from every goal at once: the cells within so many
/// steps of a goal, over the cells that may be walked on, a step further
/// each [`Wave::advance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wave {
    /// The cells reached so far.
    reached: Rows,
}

impl Wave {
    /// A wave yet to spread: `goals` alone reached.
    pub const fn from(goals: &Rows) -> Self {
        Self { reached: *goals }
    }

    /// The cells reached so far.
    pub const fn reached(&self) -> &Rows {
        &self.reached
    }

    /// One step: every passable neighbour of a reached cell, no wall
    /// between them, reached. Whether any was -- if not, the wave has
    /// gone as far as it can.
    pub fn advance(&mut self, passable: &Rows, walls: &Walls) -> bool {
        let wider = spread(&self.reached, walls);
        let mut grew = 0;
        for ((reached, &wider), &passable) in self.reached.iter_mut().zip(&wider).zip(passable) {
            let new = wider & passable & !*reached;
            *reached |= new;
            grew |= new;
        }
        grew != 0
    }
}

/// A walker on `from`'s next step to the nearest cell of `goals` over
/// the cells of `passable`, through no wall of `walls`, and how many
/// steps away that goal is: waves
/// spread from every goal until one comes beside the walker, which steps
/// into it -- of the neighbours reached together, the `pick`-th, round
/// and round, a random number, so walkers do not all lean one way.
/// `from` itself need not be passable, nor is it a goal. `None` if no
/// goal can be walked to.
pub fn step_towards(passable: &Rows, walls: &Walls, goals: &Rows, from: Cell, pick: u64) -> Option<Path> {
    let mut beside: Rows = [0; SIDE];
    beside[from.y as usize] = 1 << from.x;
    beside = spread(&beside, walls);
    beside[from.y as usize] &= !(1 << from.x);
    let mut wave = Wave::from(goals);
    wave.reached[from.y as usize] &= !(1 << from.x);
    let mut steps = 1;
    loop {
        let (mut arrived, mut count): (Rows, u32) = ([0; SIDE], 0);
        for ((arrived, &beside), &reached) in arrived.iter_mut().zip(&beside).zip(&wave.reached) {
            *arrived = beside & reached;
            count += arrived.count_ones();
        }
        if count > 0 {
            let mut left = pick % count as u64;
            for (y, &row) in arrived.iter().enumerate() {
                let mut bits = row;
                while bits != 0 {
                    if left == 0 {
                        return Some(Path { first: Cell { x: bits.trailing_zeros() as u8, y: y as u8 }, steps });
                    }
                    left -= 1;
                    bits &= bits - 1;
                }
            }
        }
        if !wave.advance(passable, walls) {
            return None;
        }
        steps += 1;
    }
}
