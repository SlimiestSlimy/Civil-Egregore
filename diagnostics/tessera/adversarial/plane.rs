//! From one adversarial window to the whole plane: each of the sixteen
//! window positions its own one of the window's sixteen variants, so
//! none is a copy of another (`docs/lab.md`, "`diagnostics/adversarial/`").

use crate::tile::Tile;
use bitmap::Bitmap;

/// A window can be turned four ways...
const ROTATION_COUNT: usize = 4;
/// ...mirrored or not...
const MIRRORING_COUNT: usize = 2;
/// ...and inverted or not...
const INVERSION_COUNT: usize = 2;
/// ...so this many variants of it fill the plane.
const VARIANT_COUNT: usize = ROTATION_COUNT * MIRRORING_COUNT * INVERSION_COUNT;

/// Where `(x, y)` of a `square_side` square lands under a variant.
fn transformed_cartesian(variant_index: usize, square_side: usize, x: usize, y: usize) -> (usize, usize) {
    let last_offset = square_side - 1;
    let (x, y) = if (variant_index / ROTATION_COUNT) % MIRRORING_COUNT == 1 { (last_offset - x, y) } else { (x, y) };
    match variant_index % ROTATION_COUNT {
        0 => (x, y),
        1 => (last_offset - y, x),
        2 => (last_offset - x, last_offset - y),
        _ => (y, last_offset - x),
    }
}

/// The plane filled with `window`'s variants, `window` being what
/// `bitmap` holds at `area`, one variant per tile of `area`'s size.
pub fn fill_the_plane(bitmap: &Bitmap, area: Tile) -> Bitmap {
    let window_side_cells = area.side_in_cells();
    let (left_x, top_y) = area.top_left_cell();
    let mut filled_bitmap = Bitmap::new();
    for (spot_index, spot_tile) in Tile::all_of_level(area.tile_level).enumerate() {
        let variant_index = spot_index % VARIANT_COUNT;
        let is_inverted = variant_index / (ROTATION_COUNT * MIRRORING_COUNT) == 1;
        let (to_x, to_y) = spot_tile.top_left_cell();
        for y in 0..window_side_cells {
            for x in 0..window_side_cells {
                let value = bitmap.get(left_x + x as u8, top_y + y as u8) != is_inverted;
                let (transformed_x, transformed_y) = transformed_cartesian(variant_index, window_side_cells, x, y);
                if value {
                    filled_bitmap.set(to_x + transformed_x as u8, to_y + transformed_y as u8);
                }
            }
        }
    }
    filled_bitmap
}
