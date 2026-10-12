//! Walls between cells: the steps that may not be taken, each way of
//! its own -- a drop is stepped down and not back up -- and a set of
//! cells spread a step through them, forwards and backwards.

use super::{Cell, Rows, SIDE, holds};

/// The step north, among a [`Walls`]' ways...
const NORTH: usize = 0;
/// ...west...
const WEST: usize = 1;
/// ...south...
const SOUTH: usize = 2;
/// ...east...
const EAST: usize = 3;
/// ...north-west...
const NORTH_WEST: usize = 4;
/// ...north-east...
const NORTH_EAST: usize = 5;
/// ...south-west...
const SOUTH_WEST: usize = 6;
/// ...and south-east.
const SOUTH_EAST: usize = 7;

/// Which of the eight ways the step `dx` across and `dy` down goes.
const fn way_of(dx: i8, dy: i8) -> usize {
    match (dx, dy) {
        (0, -1) => NORTH,
        (-1, 0) => WEST,
        (0, 1) => SOUTH,
        (1, 0) => EAST,
        (-1, -1) => NORTH_WEST,
        (1, -1) => NORTH_EAST,
        (-1, 1) => SOUTH_WEST,
        _ => SOUTH_EAST,
    }
}

/// The walls between cells of an area: steps that cannot be taken,
/// whatever the cells either side are. A step is barred out of a cell
/// one way: its way back may be open. None by default
/// (`docs/pathfinding.md`, "Walls").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Walls {
    /// For each of the eight ways, the cells no step leaves that way.
    barred: [Rows; 8],
}

impl Walls {
    /// Walls barring both ways: `east` and `south` the cells with one
    /// between them and the cell to their east, to their south.
    pub const fn new(east: Rows, south: Rows) -> Self {
        let (mut north, mut west) = ([0; SIDE], [0; SIDE]);
        let mut y = 0;
        while y < SIDE {
            west[y] = east[y] << 1;
            if y > 0 {
                north[y] = south[y - 1];
            }
            y += 1;
        }
        Self::one_way(north, west, south, east)
    }

    /// The walls each way of its own: `north`, `west`, `south` and
    /// `east` the cells no step leaves that way, and the diagonal steps
    /// they bar -- a cell's step down and east is barred unless both
    /// ways round it are open: east and then south, south and then
    /// east; the other three likewise. Past the area's edge nothing
    /// bars: no step leaves the area.
    pub const fn one_way(north: Rows, west: Rows, south: Rows, east: Rows) -> Self {
        let mut barred = [[0; SIDE]; 8];
        (barred[NORTH], barred[WEST], barred[SOUTH], barred[EAST]) = (north, west, south, east);
        let mut y = 0;
        while y < SIDE {
            let (east_above, west_above) = if y > 0 { (east[y - 1], west[y - 1]) } else { (0, 0) };
            let (east_below, west_below) = if y + 1 < SIDE { (east[y + 1], west[y + 1]) } else { (0, 0) };
            // The cell a column east is a bit up the row: its walls come a bit down to be over this cell's.
            barred[NORTH_WEST][y] = west[y] | north[y] | north[y] << 1 | west_above;
            barred[NORTH_EAST][y] = east[y] | north[y] | north[y] >> 1 | east_above;
            barred[SOUTH_WEST][y] = west[y] | south[y] | south[y] << 1 | west_below;
            barred[SOUTH_EAST][y] = east[y] | south[y] | south[y] >> 1 | east_below;
            y += 1;
        }
        Self { barred }
    }

    /// Whether a wall bars the step from `cell` to its neighbour `dx`
    /// across and `dy` down, a cell of the area too.
    pub const fn bars_step(&self, cell: Cell, dx: i8, dy: i8) -> bool {
        holds(&self.barred[way_of(dx, dy)], cell)
    }
}

/// `rows` and every cell a step from one of them comes to through
/// `walls`: each row with the rows above and below it, a column each
/// way.
pub(crate) fn spread(rows: &Rows, walls: &Walls) -> Rows {
    let barred = &walls.barred;
    let mut wider = [0; SIDE];
    let mut y = 0;
    while y < SIDE {
        let here = rows[y];
        let mut row = here | (here & !barred[EAST][y]) << 1 | (here & !barred[WEST][y]) >> 1;
        if y > 0 {
            let above = rows[y - 1];
            row |= above & !barred[SOUTH][y - 1] | (above & !barred[SOUTH_EAST][y - 1]) << 1 | (above & !barred[SOUTH_WEST][y - 1]) >> 1;
        }
        if y + 1 < SIDE {
            let below = rows[y + 1];
            row |= below & !barred[NORTH][y + 1] | (below & !barred[NORTH_EAST][y + 1]) << 1 | (below & !barred[NORTH_WEST][y + 1]) >> 1;
        }
        wider[y] = row;
        y += 1;
    }
    wider
}

/// `rows` and every cell a step from which comes to one of them
/// through `walls`: [`spread`] the other way, what a search going out
/// from its goals grows by.
pub(crate) fn spread_back(rows: &Rows, walls: &Walls) -> Rows {
    let barred = &walls.barred;
    let mut wider = [0; SIDE];
    let mut y = 0;
    while y < SIDE {
        let here = rows[y];
        let mut row = here | here >> 1 & !barred[EAST][y] | here << 1 & !barred[WEST][y];
        if y > 0 {
            let above = rows[y - 1];
            row |= above & !barred[NORTH][y] | above >> 1 & !barred[NORTH_EAST][y] | above << 1 & !barred[NORTH_WEST][y];
        }
        if y + 1 < SIDE {
            let below = rows[y + 1];
            row |= below & !barred[SOUTH][y] | below >> 1 & !barred[SOUTH_EAST][y] | below << 1 & !barred[SOUTH_WEST][y];
        }
        wider[y] = row;
        y += 1;
    }
    wider
}
