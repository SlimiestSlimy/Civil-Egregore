//! How a corpus is grown: free functions that keep nothing between
//! calls, the whole of what they make settled by their arguments
//! (`docs/lab.md`, "`corpus/`").

use utilities::rng::Rng;
use bitmap::{Bitmap, WIDTH};

/// Cells in the bitmap.
const CELLS: usize = WIDTH * WIDTH;

/// Guesses at a clear cell before scanning for one.
const GUESSES_BEFORE_SCANNING: usize = 64;

/// A bitmap grown from a seed, which settles it entirely: `density`
/// the share of the cells set, `cluster` how often a new cell lands
/// beside one already set (`docs/lab.md`, "`corpus/`").
pub(super) fn one(seed: u64, density: f64, cluster: f64) -> Bitmap {
    let wanted = (density.clamp(0.0, 1.0) * CELLS as f64) as usize;
    let cluster = cluster.clamp(0.0, 1.0);

    let mut bitmap = Bitmap::new();
    let mut rng = Rng::new(seed);

    // Unset cells beside a set one, with repeats.
    let mut edge: Vec<(u8, u8)> = Vec::new();
    let mut standing = 0;

    while standing < wanted {
        let beside = (!edge.is_empty() && rng.unit() < cluster)
            .then(|| {
                while let Some(edge_index) = (!edge.is_empty()).then(|| rng.below(edge.len() as u64) as usize) {
                    let cell = edge.swap_remove(edge_index);
                    if !bitmap.get(cell.0, cell.1) {
                        return Some(cell);
                    }
                }
                None
            })
            .flatten();

        let (x, y) = match beside {
            Some(cell) => cell,
            None => anywhere_clear(&bitmap, &mut rng),
        };

        bitmap.set(x, y);
        standing += 1;
        let (x, y) = (x as i32, y as i32);
        for (neighbour_x, neighbour_y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if (0..WIDTH as i32).contains(&neighbour_x) && (0..WIDTH as i32).contains(&neighbour_y) {
                let (neighbour_x, neighbour_y) = (neighbour_x as u8, neighbour_y as u8);
                if !bitmap.get(neighbour_x, neighbour_y) {
                    edge.push((neighbour_x, neighbour_y));
                }
            }
        }
    }

    bitmap
}

/// Any cell still clear, found by guessing and then, once guessing
/// stops paying, by looking: so a density close to 1 still finishes.
fn anywhere_clear(bitmap: &Bitmap, rng: &mut Rng) -> (u8, u8) {
    for _ in 0..GUESSES_BEFORE_SCANNING {
        let (x, y) = (rng.below(WIDTH as u64) as u8, rng.below(WIDTH as u64) as u8);
        if !bitmap.get(x, y) {
            return (x, y);
        }
    }
    // The first clear cell in reading order.
    (0..=u8::MAX).flat_map(|y| (0..=u8::MAX).map(move |x| (x, y))).find(|&(x, y)| !bitmap.get(x, y)).expect("a bitmap not already full")
}
