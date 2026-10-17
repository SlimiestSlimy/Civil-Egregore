//! The changes a search tries, each confined to the searched area.
//! Flipping single cells alone would wander; these aim at what the
//! encoders are built on -- tile boundaries, homogeneity, copies that
//! almost match, and structure no power-of-two tile lines up with.

use utilities::rng::RandomSource;
use super::cell_rect;
use crate::tile::{copy_offset, Tile, CELL_LEVEL, COPY_DIRECTIONS, FLOOR_LEVEL};
use bitmap::Bitmap;

/// The largest square a painted rectangle or a checkerboard patch
/// spans, as a share of the area's side: big enough to cross several
/// tile boundaries, small enough to stay one local change.
const PATCH_SHARE_OF_SIDE: usize = 4;

/// The longest checkerboard period tried; odd periods only, so none
/// lines up with the grid...
const LONGEST_CHECKER_PERIOD: u64 = 15;
/// ...from this shortest one up.
const SHORTEST_CHECKER_PERIOD: u64 = 3;

/// Flips one cell: set to clear, clear to set.
pub fn flip_cell(bitmap: &mut Bitmap, x: u8, y: u8) {
    if bitmap.get(x, y) {
        bitmap.clear_cell(x, y);
    } else {
        bitmap.set(x, y);
    }
}

/// A cell of `area`, anywhere.
fn some_cell(random_generator: &mut RandomSource, area: Tile) -> (u8, u8) {
    let (left_x, top_y, right_x, bottom_y) = cell_rect(area);
    (random_generator.draw_between(left_x as u64, right_x as u64) as u8, random_generator.draw_between(top_y as u64, bottom_y as u64) as u8)
}

/// A tile inside `area`, at a level from `coarsest_level` to `finest_level`.
fn some_tile_inside(random_generator: &mut RandomSource, area: Tile, coarsest_level: u8, finest_level: u8) -> Tile {
    let tile_level = random_generator.draw_between(coarsest_level as u64, finest_level as u64) as u8;
    let (x, y) = some_cell(random_generator, area);
    let level_shift = CELL_LEVEL - tile_level;
    Tile { tile_level, x: ((x as u16) >> level_shift) as u8, y: ((y as u16) >> level_shift) as u8 }
}

/// A kind of change.
pub type ChangeKind = fn(&mut RandomSource, &mut Bitmap, Tile);

/// Every kind of change, which a search picks among.
pub const CHANGE_KINDS: [ChangeKind; 5] = [flip_a_cell, flip_a_tile, paint_a_rectangle, copy_almost, xor_a_checkerboard];

/// Flips one cell of `area`, anywhere.
fn flip_a_cell(random_generator: &mut RandomSource, bitmap: &mut Bitmap, area: Tile) {
    let (x, y) = some_cell(random_generator, area);
    flip_cell(bitmap, x, y);
}

/// Every cell of one tile, which breaks or makes a homogeneous tile.
fn flip_a_tile(random_generator: &mut RandomSource, bitmap: &mut Bitmap, area: Tile) {
    let tile = some_tile_inside(random_generator, area, area.tile_level + 1, CELL_LEVEL);
    let (left_x, top_y, right_x, bottom_y) = cell_rect(tile);
    for y in top_y..=bottom_y {
        for x in left_x..=right_x {
            flip_cell(bitmap, x, y);
        }
    }
}

/// A rectangle set or cleared at any offset and size, so its edges
/// cut tiles.
fn paint_a_rectangle(random_generator: &mut RandomSource, bitmap: &mut Bitmap, area: Tile) {
    let longest_side = (area.side_in_cells() / PATCH_SHARE_OF_SIDE).max(1) as u64;
    let (x, y) = some_cell(random_generator, area);
    let (_, _, right_x, bottom_y) = cell_rect(area);
    let end_x = (x as u64 + random_generator.draw_below(longest_side)).min(right_x as u64) as i64;
    let end_y = (y as u64 + random_generator.draw_below(longest_side)).min(bottom_y as u64) as i64;
    if random_generator.draw_below(2) == 0 {
        bitmap.set_rect(x as i64, y as i64, end_x, end_y);
    } else {
        bitmap.unset_rect(x as i64, y as i64, end_x, end_y);
    }
}

/// A tile made a copy of a neighbour it could copy from, then one cell
/// of it changed: a copy that almost fits, for the copies, the copies
/// naming children and the complex tiles to argue over.
fn copy_almost(random_generator: &mut RandomSource, bitmap: &mut Bitmap, area: Tile) {
    if area.tile_level + 1 > FLOOR_LEVEL {
        return flip_a_cell(random_generator, bitmap, area);
    }
    let tile = some_tile_inside(random_generator, area, area.tile_level + 1, FLOOR_LEVEL);
    let copy_direction = random_generator.draw_below(COPY_DIRECTIONS as u64) as u8;
    let Some(source_tile) = tile.offset_by(copy_offset(false, copy_direction)) else { return };
    let tile_side_cells = tile.side_in_cells();
    let ((to_x, to_y), (from_x, from_y)) = (tile.top_left_cell(), source_tile.top_left_cell());
    for dy in 0..tile_side_cells {
        for dx in 0..tile_side_cells {
            let (source_x, source_y) = ((from_x as usize + dx) as u8, (from_y as usize + dy) as u8);
            let (copy_x, copy_y) = ((to_x as usize + dx) as u8, (to_y as usize + dy) as u8);
            if bitmap.get(source_x, source_y) != bitmap.get(copy_x, copy_y) {
                flip_cell(bitmap, copy_x, copy_y);
            }
        }
    }
    let (x, y) = some_cell(random_generator, tile);
    flip_cell(bitmap, x, y);
}

/// A patch of odd-period checkerboard XORed in: structure no tile size
/// matches.
fn xor_a_checkerboard(random_generator: &mut RandomSource, bitmap: &mut Bitmap, area: Tile) {
    let longest_side = (area.side_in_cells() / PATCH_SHARE_OF_SIDE).max(1) as u64;
    let checker_period = random_generator.draw_between(SHORTEST_CHECKER_PERIOD / 2, LONGEST_CHECKER_PERIOD / 2) * 2 + 1;
    let (start_x, start_y) = some_cell(random_generator, area);
    let (_, _, right_x, bottom_y) = cell_rect(area);
    let end_x = (start_x as u64 + random_generator.draw_below(longest_side)).min(right_x as u64) as u8;
    let end_y = (start_y as u64 + random_generator.draw_below(longest_side)).min(bottom_y as u64) as u8;
    for y in start_y..=end_y {
        for x in start_x..=end_x {
            if (x as u64 / checker_period + y as u64 / checker_period) % 2 == 1 {
                flip_cell(bitmap, x, y);
            }
        }
    }
}
