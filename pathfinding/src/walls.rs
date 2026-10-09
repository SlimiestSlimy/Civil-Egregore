//! Walls between cells: the edges a step may not cross, and a set of
//! cells spread a step through them.

use super::{Cell, Rows, SIDE, holds};

/// The walls between cells of an area: steps that cannot be taken,
/// whatever the cells either side are. A step is between two cells, so
/// a wall is kept by the upper or left one of the two, a mask a way,
/// and bars the step both ways. Walls stand east and south of cells
/// only; a diagonal step is barred unless both ways round it -- across
/// then down, and down then across -- are open, which [`Walls::new`]
/// works out once, a mask a diagonal. None by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Walls {
    /// Cells with a wall between them and the cell to their east.
    east: Rows,
    /// ...and the cell to their south.
    south: Rows,
    /// Cells whose step to the south-east is barred: a wall on either way
    /// round it.
    south_east: Rows,
    /// ...to the south-west.
    south_west: Rows,
}

impl Walls {
    /// The walls `east` and `south` of the area's cells, and the diagonal
    /// steps they bar: a cell's step down and east is barred by a wall
    /// east of it or of the cell below, or south of it or of the cell
    /// east -- its two ways round; down and west, likewise to the west.
    pub const fn new(east: Rows, south: Rows) -> Self {
        let (mut south_east, mut south_west) = ([0; SIDE], [0; SIDE]);
        let mut y = 0;
        while y < SIDE {
            // The row below's walls east; past the area's last row, none: no step leaves the area.
            let east_below = if y + 1 < SIDE { east[y + 1] } else { 0 };
            south_east[y] = east[y] | east_below | south[y] | south[y] >> 1;
            south_west[y] = (east[y] | east_below) << 1 | south[y] | south[y] << 1;
            y += 1;
        }
        Self { east, south, south_east, south_west }
    }

    /// The walls east of the area's cells.
    pub const fn east(&self) -> &Rows {
        &self.east
    }

    /// The walls south of the area's cells.
    pub const fn south(&self) -> &Rows {
        &self.south
    }

    /// Whether a wall bars the step from `cell` to its neighbour `dx`
    /// across and `dy` down, a cell of the area too.
    pub const fn bars_step(&self, cell: Cell, dx: i8, dy: i8) -> bool {
        // The upper of the two keeps the wall; of two on a row, the left.
        let (keeper, dx) = if dy < 0 || dy == 0 && dx < 0 { (Cell { x: (cell.x as i8 + dx) as u8, y: (cell.y as i8 + dy) as u8 }, -dx) } else { (cell, dx) };
        let rows = match (dx, dy != 0) {
            (1, false) => &self.east,
            (0, true) => &self.south,
            (1, true) => &self.south_east,
            _ => &self.south_west,
        };
        holds(rows, keeper)
    }
}

/// `rows` and every neighbour of its cells no wall of `walls` is before:
/// each row with the rows above and below it, a column each way.
pub(crate) fn spread(rows: &Rows, walls: &Walls) -> Rows {
    let mut wider = [0; SIDE];
    let mut y = 0;
    while y < SIDE {
        let here = rows[y];
        let mut row = here | (here & !walls.east[y]) << 1 | here >> 1 & !walls.east[y];
        if y > 0 {
            let above = rows[y - 1];
            row |= above & !walls.south[y - 1] | (above & !walls.south_east[y - 1]) << 1 | (above & !walls.south_west[y - 1]) >> 1;
        }
        if y + 1 < SIDE {
            let below = rows[y + 1];
            row |= below & !walls.south[y] | below >> 1 & !walls.south_east[y] | below << 1 & !walls.south_west[y];
        }
        wider[y] = row;
        y += 1;
    }
    wider
}
