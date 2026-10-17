//! The pattern pyramid: homogeneous and copyable exactly when the cells
//! say so, whatever was built before.

use crate::patterns::{homogeneous_value_of, PatternPyramid};
use crate::corpus::checkerboards::one_checkerboard;
use crate::corpus::one_grown;
use crate::tile::{copy_offset, Tile, COPY_DIRECTIONS, FLOOR_LEVEL};
use bitmap::Bitmap;

/// `tile`'s cells, in reading order.
fn cells(bitmap: &Bitmap, tile: Tile) -> Vec<bool> {
    let ((left_x, top_y), tile_side_cells) = (tile.top_left_cell(), tile.side_in_cells());
    (0..tile_side_cells).flat_map(|dy| (0..tile_side_cells).map(move |dx| (left_x as usize + dx, top_y as usize + dy))).map(|(x, y)| bitmap.get(x as u8, y as u8)).collect()
}

/// A ragged bitmap and an odd checkerboard: the cases every test here runs on.
fn bitmaps() -> [Bitmap; 2] {
    [one_grown(utilities::seed::given_or_fresh_seed(None), 0.20, 0.70), one_checkerboard(3)]
}

/// With the top-left quarter filled, that quarter is homogeneous and set,
/// the next one clear, and the whole bitmap neither.
#[test]
fn a_filled_quarter_is_homogeneous() {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(0, 0, 127, 127);
    let mut pattern_pyramid = PatternPyramid::new();
    pattern_pyramid.number_every_tile(&bitmap);
    let value = |tile| homogeneous_value_of(pattern_pyramid.pattern_number(tile));
    assert_eq!(value(Tile { tile_level: 1, x: 0, y: 0 }), Some(true));
    assert_eq!(value(Tile { tile_level: 1, x: 1, y: 0 }), Some(false));
    assert_eq!(value(Tile::WHOLE_BITMAP), None);
}

/// Every tile, at every level, is homogeneous by its number exactly when
/// its cells all agree -- after another bitmap was built first.
#[test]
fn homogeneity_matches_the_cells() {
    let mut pattern_pyramid = PatternPyramid::new();
    pattern_pyramid.number_every_tile(&one_checkerboard(5));
    for bitmap in bitmaps() {
        pattern_pyramid.number_every_tile(&bitmap);
        for tile in (0..=FLOOR_LEVEL).flat_map(Tile::all_of_level) {
            let cells = cells(&bitmap, tile);
            let cells_agree = cells.iter().all(|&cell| cell == cells[0]);
            assert_eq!(homogeneous_value_of(pattern_pyramid.pattern_number(tile)), cells_agree.then_some(cells[0]), "{tile:?}");
        }
    }
}

/// Every tile's copy source is the first offset, near before far, whose
/// tile holds the same cells, if any.
#[test]
fn copy_sources_agree_with_the_cells() {
    let mut pattern_pyramid = PatternPyramid::new();
    for bitmap in bitmaps() {
        pattern_pyramid.number_every_tile(&bitmap);
        for tile in (0..=FLOOR_LEVEL).flat_map(Tile::all_of_level) {
            let tile_cells = cells(&bitmap, tile);
            let first_match = [false, true].into_iter().flat_map(|is_far_copy| (0..COPY_DIRECTIONS).map(move |copy_direction| (is_far_copy, copy_direction))).find(
                |&(is_far_copy, copy_direction)| tile.offset_by(copy_offset(is_far_copy, copy_direction)).is_some_and(|source_tile| cells(&bitmap, source_tile) == tile_cells),
            );
            assert_eq!(pattern_pyramid.copy_source(tile, pattern_pyramid.pattern_number(tile)), first_match, "{tile:?}");
        }
    }
}
