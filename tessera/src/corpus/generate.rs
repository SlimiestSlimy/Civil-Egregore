//! How a corpus is grown: free functions that keep nothing between
//! calls, the whole of what they make settled by their arguments
//! (`docs/lab.md`, "`corpus/`").

use utilities::rng::RandomSource;
use utilities::bitmap::{Bitmap, WIDTH};

/// Cells in the bitmap.
const CELLS: usize = WIDTH * WIDTH;

/// Guesses at a clear cell before scanning for one.
const GUESSES_BEFORE_SCANNING: usize = 64;

/// A bitmap grown from a seed, which settles it entirely: `cell_density`
/// the share of the cells set, `cluster_weight` how often a new cell lands
/// beside one already set (`docs/lab.md`, "`corpus/`").
pub(super) fn grow_one_bitmap(seed: u64, cell_density: f64, cluster_weight: f64) -> Bitmap {
    let wanted_set_cells = (cell_density.clamp(0.0, 1.0) * CELLS as f64) as usize;
    let clamped_cluster_weight = cluster_weight.clamp(0.0, 1.0);

    let mut bitmap = Bitmap::new();
    let mut random_generator = RandomSource::new(seed);

    // Unset cells beside a set one, with repeats.
    let mut frontier_cells: Vec<(u8, u8)> = Vec::new();
    let mut set_so_far = 0;

    while set_so_far < wanted_set_cells {
        let cell_beside_set = (!frontier_cells.is_empty() && random_generator.draw_unit_float() < clamped_cluster_weight)
            .then(|| {
                while let Some(edge_index) = (!frontier_cells.is_empty()).then(|| random_generator.draw_below(frontier_cells.len() as u64) as usize) {
                    let cell = frontier_cells.swap_remove(edge_index);
                    if !bitmap.get(cell.0, cell.1) {
                        return Some(cell);
                    }
                }
                None
            })
            .flatten();

        let (x, y) = match cell_beside_set {
            Some(cell) => cell,
            None => anywhere_clear(&bitmap, &mut random_generator),
        };

        bitmap.set(x, y);
        set_so_far += 1;
        let (x, y) = (x as i32, y as i32);
        for (neighbour_x, neighbour_y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if (0..WIDTH as i32).contains(&neighbour_x) && (0..WIDTH as i32).contains(&neighbour_y) {
                let (neighbour_x, neighbour_y) = (neighbour_x as u8, neighbour_y as u8);
                if !bitmap.get(neighbour_x, neighbour_y) {
                    frontier_cells.push((neighbour_x, neighbour_y));
                }
            }
        }
    }

    bitmap
}

/// Any cell still clear, found by guessing and then, once guessing
/// stops paying, by looking: so a density close to 1 still finishes.
fn anywhere_clear(bitmap: &Bitmap, random_generator: &mut RandomSource) -> (u8, u8) {
    for _ in 0..GUESSES_BEFORE_SCANNING {
        let (x, y) = (random_generator.draw_below(WIDTH as u64) as u8, random_generator.draw_below(WIDTH as u64) as u8);
        if !bitmap.get(x, y) {
            return (x, y);
        }
    }
    // The first clear cell in reading order.
    (0..=u8::MAX).flat_map(|y| (0..=u8::MAX).map(move |x| (x, y))).find(|&(x, y)| !bitmap.get(x, y)).expect("a bitmap not already full")
}
