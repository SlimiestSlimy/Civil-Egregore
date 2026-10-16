//! Walls between cells: the steps that may not be taken, each way of
//! its own -- a drop is stepped down and not back up -- and a set of
//! cells spread a step through them, forwards and backwards.

use super::{Cell, Rows, AREA_SIDE, rows_hold};

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
    barred_rows: [Rows; 8],
}

impl Walls {
    /// Walls barring both ways: `east_walls` and `south_walls` the cells with one
    /// between them and the cell to their east, to their south.
    pub const fn new(east_walls: Rows, south_walls: Rows) -> Self {
        let (mut north_walls, mut west_walls) = ([0; AREA_SIDE], [0; AREA_SIDE]);
        let mut y = 0;
        while y < AREA_SIDE {
            west_walls[y] = east_walls[y] << 1;
            if y > 0 {
                north_walls[y] = south_walls[y - 1];
            }
            y += 1;
        }
        Self::one_way(north_walls, west_walls, south_walls, east_walls)
    }

    /// The walls each way of its own: `north_walls`, `west_walls`, `south_walls` and
    /// `east_walls` the cells no step leaves that way, and the diagonal steps
    /// they bar -- a cell's step down and east is barred unless both
    /// ways round it are open: east and then south, south and then
    /// east; the other three likewise. Past the area's edge nothing
    /// bars: no step leaves the area.
    pub const fn one_way(north_walls: Rows, west_walls: Rows, south_walls: Rows, east_walls: Rows) -> Self {
        let mut barred_rows = [[0; AREA_SIDE]; 8];
        (barred_rows[NORTH], barred_rows[WEST], barred_rows[SOUTH], barred_rows[EAST]) = (north_walls, west_walls, south_walls, east_walls);
        let mut y = 0;
        while y < AREA_SIDE {
            let (east_above, west_above) = if y > 0 { (east_walls[y - 1], west_walls[y - 1]) } else { (0, 0) };
            let (east_below, west_below) = if y + 1 < AREA_SIDE { (east_walls[y + 1], west_walls[y + 1]) } else { (0, 0) };
            // The cell a column east is a bit up the row: its walls come a bit down to be over this cell's.
            barred_rows[NORTH_WEST][y] = west_walls[y] | north_walls[y] | north_walls[y] << 1 | west_above;
            barred_rows[NORTH_EAST][y] = east_walls[y] | north_walls[y] | north_walls[y] >> 1 | east_above;
            barred_rows[SOUTH_WEST][y] = west_walls[y] | south_walls[y] | south_walls[y] << 1 | west_below;
            barred_rows[SOUTH_EAST][y] = east_walls[y] | south_walls[y] | south_walls[y] >> 1 | east_below;
            y += 1;
        }
        Self { barred_rows }
    }

    /// Whether a wall bars the step from `cell` to its neighbour `dx`
    /// across and `dy` down, a cell of the area too.
    pub const fn bars_step(&self, cell: Cell, dx: i8, dy: i8) -> bool {
        rows_hold(&self.barred_rows[way_of(dx, dy)], cell)
    }
}

/// `from_rows` and every cell a step from one of them comes to through
/// `walls`: each row with the rows above and below it, a column each
/// way.
pub(crate) fn spread_forwards(from_rows: &Rows, walls: &Walls) -> Rows {
    let barred_rows = &walls.barred_rows;
    let mut spread_rows = [0; AREA_SIDE];
    let mut y = 0;
    while y < AREA_SIDE {
        let here_row = from_rows[y];
        let mut spread_row = here_row | (here_row & !barred_rows[EAST][y]) << 1 | (here_row & !barred_rows[WEST][y]) >> 1;
        if y > 0 {
            let above_row = from_rows[y - 1];
            spread_row |= above_row & !barred_rows[SOUTH][y - 1] | (above_row & !barred_rows[SOUTH_EAST][y - 1]) << 1 | (above_row & !barred_rows[SOUTH_WEST][y - 1]) >> 1;
        }
        if y + 1 < AREA_SIDE {
            let below_row = from_rows[y + 1];
            spread_row |= below_row & !barred_rows[NORTH][y + 1] | (below_row & !barred_rows[NORTH_EAST][y + 1]) << 1 | (below_row & !barred_rows[NORTH_WEST][y + 1]) >> 1;
        }
        spread_rows[y] = spread_row;
        y += 1;
    }
    spread_rows
}

/// `to_rows` and every cell a step from which comes to one of them
/// through `walls`: [`spread_forwards`] the other way, what a search going out
/// from its goals grows by.
pub(crate) fn spread_back(to_rows: &Rows, walls: &Walls) -> Rows {
    let barred_rows = &walls.barred_rows;
    let mut spread_rows = [0; AREA_SIDE];
    let mut y = 0;
    while y < AREA_SIDE {
        let here_row = to_rows[y];
        let mut spread_row = here_row | here_row >> 1 & !barred_rows[EAST][y] | here_row << 1 & !barred_rows[WEST][y];
        if y > 0 {
            let above_row = to_rows[y - 1];
            spread_row |= above_row & !barred_rows[NORTH][y] | above_row >> 1 & !barred_rows[NORTH_EAST][y] | above_row << 1 & !barred_rows[NORTH_WEST][y];
        }
        if y + 1 < AREA_SIDE {
            let below_row = to_rows[y + 1];
            spread_row |= below_row & !barred_rows[SOUTH][y] | below_row >> 1 & !barred_rows[SOUTH_EAST][y] | below_row << 1 & !barred_rows[SOUTH_WEST][y];
        }
        spread_rows[y] = spread_row;
        y += 1;
    }
    spread_rows
}
