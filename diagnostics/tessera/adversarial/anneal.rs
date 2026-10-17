//! Simulated annealing over changes that learn: each kind drawn in
//! proportion to one plus the times it has raised the score. What is
//! maximized is the caller's (`docs/lab.md`, "`diagnostics/adversarial/`").

use super::moves::CHANGE_KINDS;
use utilities::rng::RandomSource;
use super::BitmapScore;
use crate::tile::Tile;
use bitmap::Bitmap;

/// How many bits a change may lose and still often be kept at the
/// start: about what one change moves, so early on the search crosses
/// small valleys freely. It cools linearly to nothing.
const START_TEMPERATURE: f64 = 8.0;

/// The worst bitmap a search found, and its score.
pub struct FoundWorst {
    /// The bitmap.
    pub bitmap: Bitmap,
    /// Its score.
    pub bitmap_score: BitmapScore,
}

/// A kind of change, drawn in proportion to one plus its successes.
fn pick_change_kind(random_generator: &mut RandomSource, change_successes: &[u64]) -> usize {
    let mut draw_left = random_generator.draw_below(change_successes.iter().map(|&count| count + 1).sum());
    for (change_kind, &count) in change_successes.iter().enumerate() {
        if draw_left <= count {
            return change_kind;
        }
        draw_left -= count + 1;
    }
    unreachable!("the draw is below the sum")
}

/// The best bitmap `changes_tried` changes inside `area` reach from
/// `start_bitmap`, by `score_bitmap`.
pub fn anneal_bitmap(start_bitmap: Bitmap, area: Tile, changes_tried: u64, random_generator: &mut RandomSource, score_bitmap: &mut impl FnMut(&Bitmap) -> BitmapScore) -> FoundWorst {
    let mut change_successes = [0; CHANGE_KINDS.len()];
    let mut current_bitmap = start_bitmap;
    let mut current_score = score_bitmap(&current_bitmap);
    let mut best_found = FoundWorst { bitmap: current_bitmap.clone(), bitmap_score: current_score };
    for change_number in 0..changes_tried {
        let temperature = START_TEMPERATURE * (1.0 - change_number as f64 / changes_tried as f64);
        let change_kind = pick_change_kind(random_generator, &change_successes);
        let mut next_bitmap = current_bitmap.clone();
        CHANGE_KINDS[change_kind](random_generator, &mut next_bitmap, area);
        let next_score = score_bitmap(&next_bitmap);
        let score_gain = (next_score.gap - current_score.gap) as f64;
        if score_gain > 0.0 {
            change_successes[change_kind] += 1;
        }
        if score_gain >= 0.0 || (temperature > 0.0 && random_generator.draw_unit_float() < (score_gain / temperature).exp()) {
            current_bitmap = next_bitmap;
            current_score = next_score;
            if current_score.gap > best_found.bitmap_score.gap {
                best_found = FoundWorst { bitmap: current_bitmap.clone(), bitmap_score: current_score };
            }
        }
    }
    best_found
}
