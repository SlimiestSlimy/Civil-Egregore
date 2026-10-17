//! Drawing on a bitmap: rectangles and circles, by their shape, clamped
//! to the bitmap (`docs/reference.md`, "bitmap_drawing.rs").

use crate::bitmap::{Bitmap, HEIGHT, WIDTH};

/// `x` pulled onto the bitmap, a column.
fn clamped_column(x: i64) -> u8 {
    x.clamp(0, WIDTH as i64 - 1) as u8
}

/// `y` pulled onto the bitmap, a row.
fn clamped_row(y: i64) -> u8 {
    y.clamp(0, HEIGHT as i64 - 1) as u8
}

impl Bitmap {
    /// Sets every cell of the inclusive rectangle with the two given
    /// opposite corners, named either way round, clamped to the bitmap.
    pub fn set_rect(&mut self, first_x: i64, first_y: i64, second_x: i64, second_y: i64) {
        self.for_each_in_rect(first_x, first_y, second_x, second_y, |bitmap, x, y| bitmap.set(x, y));
    }

    /// Clears every cell of the inclusive rectangle with the two given
    /// opposite corners, named either way round, clamped to the bitmap.
    pub fn unset_rect(&mut self, first_x: i64, first_y: i64, second_x: i64, second_y: i64) {
        self.for_each_in_rect(first_x, first_y, second_x, second_y, |bitmap, x, y| bitmap.clear_cell(x, y));
    }

    /// Visits every cell of a rectangle given by two opposite corners in
    /// any order, clamped to the bitmap: shared by setting and clearing,
    /// so the two cannot disagree about what a rectangle is.
    fn for_each_in_rect(
        &mut self,
        first_x: i64,
        first_y: i64,
        second_x: i64,
        second_y: i64,
        mut visit_cell: impl FnMut(&mut Self, u8, u8),
    ) {
        let (left_column, right_column) = (clamped_column(first_x.min(second_x)), clamped_column(first_x.max(second_x)));
        let (top_row, bottom_row) = (clamped_row(first_y.min(second_y)), clamped_row(first_y.max(second_y)));
        for y in top_row..=bottom_row {
            for x in left_column..=right_column {
                visit_cell(self, x, y);
            }
        }
    }

    /// Sets every cell whose centre lies within `radius_in_cells` of
    /// `(centre_x, centre_y)`.
    pub fn set_circle(&mut self, centre_x: i64, centre_y: i64, radius_in_cells: i64) {
        self.for_each_in_circle(centre_x, centre_y, radius_in_cells, |bitmap, x, y| bitmap.set(x, y));
    }

    /// Clears every cell whose centre lies within `radius_in_cells` of
    /// `(centre_x, centre_y)`.
    pub fn unset_circle(&mut self, centre_x: i64, centre_y: i64, radius_in_cells: i64) {
        self.for_each_in_circle(centre_x, centre_y, radius_in_cells, |bitmap, x, y| bitmap.clear_cell(x, y));
    }

    /// Visits every cell whose centre lies within `radius_in_cells` of
    /// `(centre_x, centre_y)`, by walking the bounding box and testing
    /// squared distance, so no square root is taken. A negative radius
    /// draws nothing.
    fn for_each_in_circle(&mut self, centre_x: i64, centre_y: i64, radius_in_cells: i64, mut visit_cell: impl FnMut(&mut Self, u8, u8)) {
        if radius_in_cells < 0 {
            return;
        }
        let radius_squared = radius_in_cells * radius_in_cells;
        let (left_column, right_column) = (clamped_column(centre_x - radius_in_cells), clamped_column(centre_x + radius_in_cells));
        let (top_row, bottom_row) = (clamped_row(centre_y - radius_in_cells), clamped_row(centre_y + radius_in_cells));
        for y in top_row..=bottom_row {
            let dy = y as i64 - centre_y;
            for x in left_column..=right_column {
                let dx = x as i64 - centre_x;
                if dx * dx + dy * dy <= radius_squared {
                    visit_cell(self, x, y);
                }
            }
        }
    }
}
