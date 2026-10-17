//! Fine tests: one bitmap each, drawn by hand or grown from the seed
//! every other run uses -- read without counting a use, as these run far
//! more often than anything measured -- so a failure points at one
//! small case, reproduced by pinning the seed it printed
//! (`CIVIL_EGREGORE_SEED=<seed>`). Each also pins down something specific the
//! bitmap is meant to exercise.
//!
//! `cargo test --test fine`

mod tests;

use utilities::bitmap::Bitmap;
use tests::check_bitmap;
use tests::examination::tree_of;
use tests::saved::every_saved;
use tests::tree_stats::TreeStats;
use tessera::encode_bitmap;
use tessera::corpus::{one_grown, one_laid_out, CITY_PLANS};
use tessera::tile::{Tile, CELL_LEVEL, FLOOR_LEVEL};
use tessera::tree::Node;

/// The tile at `tile_level` holding the cell `cell`.
fn ancestor_tile(cell: Tile, tile_level: u8) -> Tile {
    let level_shift = CELL_LEVEL - tile_level;
    Tile { tile_level, x: ((cell.x as u16) >> level_shift) as u8, y: ((cell.y as u16) >> level_shift) as u8 }
}

/// An empty bitmap's tree is one tile at the top, but its stream is its
/// binary count tree, in 2 bits: fewer than the tree's 8.
#[test]
fn all_clear_is_an_empty_binary_count_tree_in_two_bits() {
    let bitmap = Bitmap::new();
    // The stream's mode bit + the set cell count, zero, in 1 bit.
    assert_eq!(encode_bitmap(&bitmap).len(), 2);
    assert_eq!(tree_of(&bitmap).get(Tile::WHOLE_BITMAP), Node::ComplexTile { size_offset: 0 });
    check_bitmap(&bitmap, "all clear");
}

/// One set cell, wherever it is, is a binary count tree of 20 bits, and passes
/// every check: a cell alone is the binary count tree's best case.
#[test]
fn one_cell_is_a_binary_count_tree_in_twenty_bits() {
    for (x, y) in [(0, 0), (255, 255), (0, 255), (131, 77)] {
        let mut bitmap = Bitmap::new();
        bitmap.set(x, y);
        // The stream's mode bit + the count, 1, as 2 in Elias gamma (3
        // bits) + one bit a halving, 16 of them, for which half holds it.
        assert_eq!(encode_bitmap(&bitmap).len(), 20, "the cell at ({x}, {y})");
        check_bitmap(&bitmap, &format!("one cell at ({x}, {y})"));
    }
}

/// Two set cells in opposite halves of the Morton order are a count
/// split of 36 bits, and pass every check.
#[test]
fn two_cells_apart_are_a_binary_count_tree_in_thirty_six_bits() {
    let mut bitmap = Bitmap::new();
    bitmap.set(3, 5);
    bitmap.set(250, 240);
    // The stream's mode bit + the count, 2, as 3 in Elias gamma (3 bits)
    // + how many of the 2 are in the first half, 1 of 0..=2 in truncated
    // binary (2 bits) + 15 halvings under each of the two halves, a bit
    // each.
    assert_eq!(encode_bitmap(&bitmap).len(), 36);
    check_bitmap(&bitmap, "two cells apart");
}

/// A full bitmap is one tile at the top, in 8 bits, and passes every
/// check.
#[test]
fn all_set_is_one_tile_in_eight_bits() {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(0, 0, 255, 255);
    // The stream's mode bit + 3 start level bits (0) + leaf + bind + the
    // plain-tile bit + 1 value bit
    assert_eq!(encode_bitmap(&bitmap).len(), 8);
    check_bitmap(&bitmap, "all set");
}

/// Every saved adversarial bitmap passes every check: each is a hard
/// case, kept for working on Tessera against.
#[test]
fn adversarial_bitmaps_pass_every_check() {
    let saved_bitmaps = every_saved();
    assert!(!saved_bitmaps.is_empty(), "no saved adversarial bitmaps in tests/saved");
    for (bitmap_name, bitmap) in saved_bitmaps {
        check_bitmap(&bitmap, &bitmap_name);
    }
}

/// The top-right quarter repeating the top-left one matches it, and is
/// placed as a near copy of it.
#[test]
fn a_repeated_quarter_is_a_near_copy() {
    let mut bitmap = Bitmap::new();
    bitmap.set_circle(64, 64, 40);
    bitmap.set_circle(192, 64, 40);
    let right_quarter = Tile { tile_level: 1, x: 1, y: 0 };
    // Near direction 3 is the neighbour to the left.
    assert_eq!(tree_of(&bitmap).get(right_quarter), Node::CopiedTile { is_far_copy: false, copy_direction: 3, names_children: false });
    check_bitmap(&bitmap, "a repeated quarter");
}

/// Overlapping rectangles and circles, set and cleared, pass every
/// check.
#[test]
fn rectangles_and_circles_round_trip() {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(10, 10, 40, 30);
    bitmap.set_rect(100, 3, 200, 90);
    bitmap.set_circle(180, 180, 25);
    bitmap.unset_circle(150, 50, 20);
    check_bitmap(&bitmap, "rectangles and circles");
}

/// A regular city forms complex tiles, and passes every check.
#[test]
fn one_city_round_trips_with_complex_tiles() {
    let bitmap = one_laid_out(tests::fresh_test_seed(), &CITY_PLANS[0]);
    assert!(TreeStats::measure_tree(&tree_of(&bitmap)).complex_tiles > 0, "a city this regular forms complex tiles");
    check_bitmap(&bitmap, "one city");
}

/// A middling, ragged bitmap passes every check.
#[test]
fn one_ragged_bitmap_round_trips() {
    check_bitmap(&one_grown(tests::fresh_test_seed(), 0.20, 0.70), "one middling ragged bitmap");
}

/// A complex tile says every cell under it: where one square of a regular area holds
/// a lone cell, the areas around it are still complex tiles, and the
/// lone cell is said on its own.
#[test]
fn a_lone_cell_leaves_the_areas_around_it_complex_tiles() {
    // The top-left 64x64: four 32x32s of 16x16 squares, one square set in
    // each, a different one each time -- no 32x32 is homogeneous or a
    // copy of another, so each is one complex tile at 16x16 resolution.
    // One clear square holds a lone set cell, which no resolution coarser
    // than 1x1 can say: its 32x32 is no complex tile, and the lone cell's
    // 8x8 says its cells as a cell list.
    /// The side of one square, in cells.
    const SQUARE_SIDE: i64 = 16;
    let mut bitmap = Bitmap::new();
    for (square_x, square_y) in [(0, 0), (3, 0), (0, 3), (3, 3)] {
        let (x, y) = (square_x * SQUARE_SIDE, square_y * SQUARE_SIDE);
        bitmap.set_rect(x, y, x + SQUARE_SIDE - 1, y + SQUARE_SIDE - 1);
    }
    let lone_cell = Tile { tile_level: 8, x: 21, y: 5 };
    bitmap.set(lone_cell.x, lone_cell.y);

    let written_tree = tree_of(&bitmap);
    let lone_cells_32x32 = ancestor_tile(lone_cell, 3);
    for quarter_tile in ancestor_tile(lone_cell, 2).child_tiles().into_iter().filter(|&candidate_quarter| candidate_quarter != lone_cells_32x32) {
        assert_eq!(written_tree.get(quarter_tile), Node::ComplexTile { size_offset: 1 }, "{quarter_tile:?}");
    }
    for tile_level in 3..FLOOR_LEVEL - 1 {
        assert_eq!(written_tree.get(ancestor_tile(lone_cell, tile_level)), Node::PlainDivide);
    }
    assert_eq!(written_tree.get(ancestor_tile(lone_cell, FLOOR_LEVEL - 1)), Node::CellList);
    check_bitmap(&bitmap, "a lone cell among complex tiles");
}
