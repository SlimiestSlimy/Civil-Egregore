//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod bitmap {
    //! The bitmap: its cells, squares, rectangles and circles.
    //!
    //! `cargo test`

    use bitmap::{Bitmap, HEIGHT, WIDTH};

    /// A bitmap is empty until a cell is set, and again once it is unset.
    #[test]
    fn empty_until_a_cell_is_set() {
        let mut bitmap = Bitmap::new();
        assert!(bitmap.is_empty());
        bitmap.set(255, 255);
        assert!(!bitmap.is_empty());
        bitmap.clear_cell(255, 255);
        assert!(bitmap.is_empty());
    }

    /// Square fills agree with the same squares drawn cell by cell.
    #[test]
    fn squares_agree_with_their_cells() {
        let mut bitmap = Bitmap::new();
        bitmap.set_rect(8, 8, 15, 15);
        bitmap.set_rect(16, 0, 19, 3);
        bitmap.set(24, 4);
        let places: Vec<usize> = bitmap.set_cells_in_tile((16, 0), 8).collect();
        assert_eq!(places, (0..16).collect::<Vec<_>>(), "the 4x4 at (16, 0) is the first 16 of its 8x8");
        assert_eq!(bitmap.set_cells_in_tile((24, 4), 1).collect::<Vec<_>>(), vec![0]);
        let mut placed_bitmap = Bitmap::new();
        for place in bitmap.set_cells_in_tile((0, 0), 32) {
            placed_bitmap.set_in_tile((0, 0), place);
        }
        assert!((0..32).all(|y| (0..32).all(|x| placed_bitmap.get(x, y) == bitmap.get(x, y))));
        let mut filled_bitmap = Bitmap::new();
        filled_bitmap.set_tile((8, 8), 8);
        filled_bitmap.set_tile((16, 0), 4);
        filled_bitmap.set_tile((24, 4), 1);
        assert!((0..=u8::MAX).all(|y| (0..=u8::MAX).all(|x| filled_bitmap.get(x, y) == bitmap.get(x, y))));
    }

    /// A new bitmap has nothing set.
    #[test]
    fn starts_empty() {
        let bitmap = Bitmap::new();
        assert_eq!(bitmap.count_set(), 0);
        assert!(!bitmap.get(0, 0));
        assert!(!bitmap.get(255, 255));
    }

    /// Setting then unsetting one cell leaves it, and the count, as before.
    #[test]
    fn set_and_unset_single_bit() {
        let mut bitmap = Bitmap::new();
        bitmap.set(10, 20);
        assert!(bitmap.get(10, 20));
        assert_eq!(bitmap.count_set(), 1);
        bitmap.clear_cell(10, 20);
        assert!(!bitmap.get(10, 20));
        assert_eq!(bitmap.count_set(), 0);
    }

    /// A rectangle includes both corners, whichever way round they are
    /// named.
    #[test]
    fn rect_is_inclusive_and_order_independent() {
        let mut bitmap = Bitmap::new();
        bitmap.set_rect(5, 5, 2, 2);
        assert_eq!(bitmap.count_set(), 16);
        assert!((2..=5).all(|y| (2..=5).all(|x| bitmap.get(x, y))));
        bitmap.unset_rect(2, 2, 5, 5);
        assert_eq!(bitmap.count_set(), 0);
    }

    /// A rectangle hanging off the edge is clamped to the bitmap.
    #[test]
    fn rect_clamps_to_bounds() {
        let mut bitmap = Bitmap::new();
        bitmap.set_rect(-10, -10, 1, 1);
        assert_eq!(bitmap.count_set(), 4);
    }

    /// A circle holds its centre and cells at its radius, not its bounding
    /// box's corners.
    #[test]
    fn circle_includes_centre_and_excludes_far_corners() {
        let mut bitmap = Bitmap::new();
        bitmap.set_circle(128, 128, 5);
        assert!(bitmap.get(128, 128));
        assert!(bitmap.get(133, 128));
        assert!(!bitmap.get(134, 128));
        assert!(!bitmap.get(133, 133));
    }

    /// Unsetting a circle clears what setting it set.
    #[test]
    fn unset_circle_clears_previously_set_bits() {
        let mut bitmap = Bitmap::new();
        bitmap.set_circle(50, 50, 10);
        assert!(bitmap.count_set() > 0);
        bitmap.unset_circle(50, 50, 10);
        assert_eq!(bitmap.count_set(), 0);
    }

    /// Resetting clears every cell.
    #[test]
    fn reset_clears_everything() {
        let mut bitmap = Bitmap::new();
        bitmap.set_rect(0, 0, 255, 255);
        assert_eq!(bitmap.count_set() as usize, WIDTH * HEIGHT);
        bitmap.reset();
        assert_eq!(bitmap.count_set(), 0);
    }

    /// Cells, rectangles and circles drawn and cleared one over
    /// another, anywhere and past the bitmap's edges, leave every cell
    /// as working each out by its coordinates does: a rectangle its
    /// corners pulled onto the bitmap, a circle the cells no farther
    /// than its radius from its centre.
    #[test]
    fn shapes_drawn_are_their_cells_worked_out_one_by_one() {
        let mut random_numbers = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
        for _ in 0..20 {
            let (mut bitmap, mut expected_cells) = (Bitmap::new(), vec![false; WIDTH * HEIGHT]);
            for _ in 0..random_numbers.draw_between(1, 12) {
                let is_setting = random_numbers.draw_below(3) != 0;
                let near_coordinate = |passed_random_numbers: &mut utilities::rng::RandomSource| passed_random_numbers.draw_below(400) as i64 - 72;
                let is_covered: Box<dyn Fn(i64, i64) -> bool> = match random_numbers.draw_below(3) {
                    0 => {
                        let (x, y) = (random_numbers.draw_below(256) as u8, random_numbers.draw_below(256) as u8);
                        if is_setting { bitmap.set(x, y) } else { bitmap.clear_cell(x, y) }
                        Box::new(move |cell_x, cell_y| (cell_x, cell_y) == (i64::from(x), i64::from(y)))
                    }
                    1 => {
                        let (first_x, first_y, second_x, second_y) = (near_coordinate(&mut random_numbers), near_coordinate(&mut random_numbers), near_coordinate(&mut random_numbers), near_coordinate(&mut random_numbers));
                        if is_setting { bitmap.set_rect(first_x, first_y, second_x, second_y) } else { bitmap.unset_rect(first_x, first_y, second_x, second_y) }
                        let onto_bitmap = |unclamped_coordinate: i64| unclamped_coordinate.clamp(0, 255);
                        Box::new(move |x, y| (onto_bitmap(first_x.min(second_x))..=onto_bitmap(first_x.max(second_x))).contains(&x) && (onto_bitmap(first_y.min(second_y))..=onto_bitmap(first_y.max(second_y))).contains(&y))
                    }
                    _ => {
                        let (centre_x, centre_y, radius_in_cells) = (near_coordinate(&mut random_numbers), near_coordinate(&mut random_numbers), random_numbers.draw_below(120) as i64 - 4);
                        if is_setting { bitmap.set_circle(centre_x, centre_y, radius_in_cells) } else { bitmap.unset_circle(centre_x, centre_y, radius_in_cells) }
                        Box::new(move |x, y| radius_in_cells >= 0 && (x - centre_x).pow(2) + (y - centre_y).pow(2) <= radius_in_cells * radius_in_cells)
                    }
                };
                for (x, y) in (0..256i64).flat_map(|y| (0..256i64).map(move |x| (x, y))).filter(|&(x, y)| is_covered(x, y)) {
                    expected_cells[y as usize * WIDTH + x as usize] = is_setting;
                }
            }
            for (x, y) in (0..=255u8).flat_map(|y| (0..=255u8).map(move |x| (x, y))) {
                assert_eq!(bitmap.get(x, y), expected_cells[y as usize * WIDTH + x as usize], "cell ({x}, {y})");
            }
            assert_eq!(bitmap.count_set() as usize, expected_cells.iter().filter(|&&cell_is_set| cell_is_set).count());
        }
    }
}

mod morton {
    //! Morton order: numbered as drawn, and undone exactly.
    //!
    //! `cargo test`

    use bitmap::morton::{morton_coordinates, morton_index};

    /// The first sixteen cells are numbered as the module doc draws
    /// them, and the last cell last.
    #[test]
    fn numbers_the_first_sixteen_as_drawn() {
        /// The module doc's drawing, row by row.
        const FIRST_SIXTEEN: [[usize; 4]; 4] = [[0, 1, 4, 5], [2, 3, 6, 7], [8, 9, 12, 13], [10, 11, 14, 15]];
        for (y, drawn_row) in FIRST_SIXTEEN.iter().enumerate() {
            for (x, &drawn_morton_index) in drawn_row.iter().enumerate() {
                assert_eq!(morton_index(x as u8, y as u8), drawn_morton_index);
            }
        }
        assert_eq!(morton_index(u8::MAX, u8::MAX), u16::MAX as usize);
    }

    /// Every coordinate pair comes back from its own Morton index.
    #[test]
    fn coordinates_invert_the_index() {
        for y in 0..=u8::MAX {
            for x in 0..=u8::MAX {
                assert_eq!(morton_coordinates(morton_index(x, y)), (x, y));
            }
        }
    }
}

mod window {
    //! Tiles: a Morton word turned into rows and back, and windows at any
    //! cell put together from four tiles, checked cell by cell against the
    //! Morton index and the coordinates.
    //!
    //! `cargo test`

    use bitmap::morton::morton_index;
    use utilities::rng::RandomSource;
    use bitmap::window::{left_columns, morton_from_rows, rows_from_morton, top_rows, window, WORD_TILE_SIDE};

    /// Whether the cell `(x, y)` of a row-by-row tile is set.
    fn bit_at(row_ordered_bits: u64, x: u32, y: u32) -> bool {
        row_ordered_bits >> (y * WORD_TILE_SIDE + x) & 1 == 1
    }

    /// A Morton word's cell `(x, y)` lands at bit `y * 8 + x` of its rows,
    /// and back.
    #[test]
    fn morton_words_turn_into_rows_and_back() {
        let mut random_numbers = RandomSource::new(utilities::seed::counted_seed());
        for _ in 0..1000 {
            let word = random_numbers.next_draw();
            let row_ordered_bits = rows_from_morton(word);
            for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
                assert_eq!(bit_at(row_ordered_bits, x, y), word >> morton_index(x as u8, y as u8) & 1 == 1, "({x}, {y})");
            }
            assert_eq!(morton_from_rows(row_ordered_bits), word);
        }
    }

    /// A window at any offset into four tiles is the 8x8 cells there.
    #[test]
    fn windows_cut_the_cells_from_four_tiles() {
        let mut random_numbers = RandomSource::new(utilities::seed::counted_seed());
        for _ in 0..200 {
            let tiles = [[random_numbers.next_draw(), random_numbers.next_draw()], [random_numbers.next_draw(), random_numbers.next_draw()]];
            let cell = |x: u32, y: u32| bit_at(tiles[(y / 8) as usize][(x / 8) as usize], x % 8, y % 8);
            for (columns_across, rows_down) in (0..8).flat_map(|row_down| (0..8).map(move |column_across| (column_across, row_down))) {
                let cut_window = window(tiles, columns_across, rows_down);
                for (x, y) in (0..8).flat_map(|y| (0..8).map(move |x| (x, y))) {
                    assert_eq!(bit_at(cut_window, x, y), cell(columns_across + x, rows_down + y), "window at ({columns_across}, {rows_down}), cell ({x}, {y})");
                }
            }
        }
    }

    /// The masks keep what they say.
    #[test]
    fn masks_keep_columns_and_rows() {
        assert_eq!((left_columns(0), left_columns(8), left_columns(3) & 0xff), (0, u64::MAX, 0b111));
        assert_eq!((top_rows(0), top_rows(8), top_rows(2)), (0, u64::MAX, 0xffff));
    }
}
