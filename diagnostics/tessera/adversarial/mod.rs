//! Adversarial bitmaps: searches for the bitmaps an encoder does worst
//! on, by any score the caller gives -- a window first, then the whole
//! plane, each a simulated annealing. See `docs/testing_protocol.md`.
//!
//! Function by function: `docs/lab.md`, "`diagnostics/adversarial/`".

mod anneal;
mod moves;
mod plane;
pub mod worst;

pub use anneal::FoundWorst;
use anneal::anneal_bitmap;
use utilities::rng::RandomSource;

use crate::tile::Tile;
use bitmap::Bitmap;

/// What one bitmap scored.
#[derive(Clone, Copy, Debug)]
pub struct BitmapScore {
    /// What the search maximizes: Tessera's bits less what they are held
    /// against.
    pub gap: i64,
    /// Tessera's bits for the whole bitmap.
    pub tessera_bits: u64,
}

/// The searched window: a 64x64, the top left one.
pub const WINDOW: Tile = Tile { tile_level: 2, x: 0, y: 0 };

/// How long a search runs: the changes tried from each start, in each
/// stage. One long search cools slowly -- the temperature falls over all
/// its changes -- so it can settle deeper than many short ones, each of
/// which starts hot again.
#[derive(Clone, Copy, Debug)]
pub struct SearchEffort {
    /// Changes tried on a window, from each start.
    pub window: u64,
    /// Changes tried on the whole plane, from each start.
    pub whole_bitmap: u64,
}

impl Default for SearchEffort {
    /// A quick search: 400 changes a window start, 100 a plane start.
    fn default() -> Self {
        Self { window: 400, whole_bitmap: 100 }
    }
}

/// One in this many of a noisy start's cells is set: half, the most
/// disordered.
const NOISE_DENSITY_DIVISOR: u64 = 2;

/// `area` filled with noise, the rest clear.
fn noisy_area(random_generator: &mut RandomSource, area: Tile) -> Bitmap {
    let mut bitmap = Bitmap::new();
    let (left_x, top_y, right_x, bottom_y) = cell_rect(area);
    for y in top_y..=bottom_y {
        for x in left_x..=right_x {
            if random_generator.draw_below(NOISE_DENSITY_DIVISOR) == 0 {
                bitmap.set(x, y);
            }
        }
    }
    bitmap
}

/// The best of `named_starts`, each annealed in `area` by `score_bitmap`, and which
/// start it came from.
fn best_of(
    named_starts: Vec<(&'static str, Bitmap)>,
    area: Tile,
    changes_tried: u64,
    random_generator: &mut RandomSource,
    score_bitmap: &mut impl FnMut(&Bitmap, Tile) -> BitmapScore,
) -> (FoundWorst, &'static str) {
    named_starts
        .into_iter()
        .map(|(start_name, start_bitmap)| (anneal_bitmap(start_bitmap, area, changes_tried, random_generator, &mut |bitmap| score_bitmap(bitmap, area)), start_name))
        .max_by_key(|(annealed_start, _)| annealed_start.bitmap_score.gap)
        .expect("a start")
}

/// What one search found, and which start each stage's best came from.
pub struct SearchOutcome {
    /// The worst found in the window stage, searching one small tile.
    pub window: FoundWorst,
    /// Which start the window stage's worst came from.
    pub window_from: &'static str,
    /// The worst found over the whole bitmap.
    pub worst: FoundWorst,
    /// Which start that came from.
    pub worst_from: &'static str,
}

/// One whole search, from its own seed, as long as `search_effort` says, by
/// `score_bitmap` -- which is told the area searched -- carrying on from
/// `worst`, if any.
pub fn whole_search(seed: u64, worst: Option<Bitmap>, search_effort: SearchEffort, score_bitmap: &mut impl FnMut(&Bitmap, Tile) -> BitmapScore) -> SearchOutcome {
    let mut random_generator = RandomSource::new(seed);
    let window_starts = vec![("clear", Bitmap::new()), ("noise", noisy_area(&mut random_generator, WINDOW))];
    let (window, window_from) = best_of(window_starts, WINDOW, search_effort.window, &mut random_generator, score_bitmap);

    let whole_bitmap_tile = Tile::WHOLE_BITMAP;
    let mut plane_starts =
        vec![("window variants", plane::fill_the_plane(&window.bitmap, WINDOW)), ("noise", noisy_area(&mut random_generator, whole_bitmap_tile))];
    plane_starts.extend(worst.map(|bitmap| ("worst", bitmap)));
    let (worst, worst_from) = best_of(plane_starts, whole_bitmap_tile, search_effort.whole_bitmap, &mut random_generator, score_bitmap);
    SearchOutcome { window, window_from, worst, worst_from }
}

/// Searches run at once by [`search_at_once`], one a core.
pub const SEARCHES_AT_ONCE: u64 = 4;

/// [`SEARCHES_AT_ONCE`] whole searches at once, one a thread, the `i`th
/// from seed `seed + i`, each carrying on from `worst`, if any, and
/// scoring with a score of its own that `make_score` makes -- one per
/// thread, so each may hold its own encoders. What each found, in order.
pub fn search_at_once<S: FnMut(&Bitmap, Tile) -> BitmapScore>(
    seed: u64,
    worst: Option<Bitmap>,
    search_effort: SearchEffort,
    make_score: &(impl Fn() -> S + Sync),
) -> Vec<SearchOutcome> {
    std::thread::scope(|thread_scope| {
        let search_threads: Vec<_> = (0..SEARCHES_AT_ONCE)
            .map(|search_index| {
                let worst = worst.clone();
                thread_scope.spawn(move || whole_search(seed.wrapping_add(search_index), worst, search_effort, &mut make_score()))
            })
            .collect();
        search_threads.into_iter().map(|search_thread| search_thread.join().expect("a search")).collect()
    })
}

/// `area`'s cells as an inclusive rectangle: left, top, right, bottom.
pub(crate) fn cell_rect(area: Tile) -> (u8, u8, u8, u8) {
    let (left_x, top_y) = area.top_left_cell();
    let last_offset = (area.side_in_cells() - 1) as u8;
    (left_x, top_y, left_x + last_offset, top_y + last_offset)
}
