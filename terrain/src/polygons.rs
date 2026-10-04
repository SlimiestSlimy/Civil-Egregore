//! The land as polygons: the world cut into closed shapes that share
//! borders and never overlap, each ocean or land, each land one a
//! plain at a level of its own.
//!
//! A polygon is the cells nearer one site than any other. The sites
//! are one to each square of a grid, placed by lot in the square's
//! middle half, so a cell's nearest -- and every one little farther --
//! is among the 25 squares about it: any cell's polygon follows from the seed and the cell alone.
//! The cell is first moved by broad noise, which bends the borders.
//! Near a border the levels of the polygons about it are mixed, over
//! the edge's width -- half a square's side at most, past which a site
//! beyond the 25 would count: a ramp, a shore, or -- the edge narrow --
//! a cliff.

use crate::{noise, Shape, ONE};
use utilities::hash::{mix, GOLDEN_RATIO};

/// How the land's polygons lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Polygons {
    /// The cells along a square of the sites' grid, as a power of two:
    /// about a polygon's breadth. 0, and there are none: the land is
    /// its rise.
    pub span: u32,
    /// The share of the polygons that are ocean, of [`ONE`].
    pub ocean: u64,
    /// The most a land polygon's plain stands over the ocean, in heights.
    pub levels: u64,
    /// The cells from a border over which the levels about it are mixed.
    pub edge: u64,
    /// How far the borders are bent, beside a square's side, of [`ONE`].
    pub warp: u64,
}

impl Polygons {
    /// None: the land is its rise.
    pub const NONE: Self = Self { span: 0, ocean: ONE / 2, levels: 200, edge: 512, warp: ONE / 4 };
}

/// What the sites are drawn by: mixed with the seed, so that they lie apart from all else.
const SITES_SALT: u64 = 0x706F_6C79_676F_6E73;
/// The number the noise that bends the borders is drawn by, across; down is the next.
const WARP_INDEX: u32 = 30;

/// A site near a cell.
#[derive(Clone, Copy)]
struct Site {
    /// Cells from the cell to it.
    away: u64,
    /// Whether its polygon is land.
    land: bool,
    /// How far its plain stands over the ocean, if it is land.
    level: u64,
    /// Its lot: what tells it from every other.
    lot: u64,
}

/// The sites of the 25 squares about the cell `(x, y)`, the nearest first.
fn sites(shape: &Shape, seed: u64, x: u32, y: u32) -> [Site; 25] {
    let polygons = &shape.polygons;
    let span = polygons.span.clamp(6, 24);
    let side = 1i64 << span;
    // The cell, moved by broad noise: what bends the borders.
    let bend = ((side as u64 * polygons.warp) >> 16) as i64;
    let moved = |index: u32| ((noise(seed, index, span - 2, x, y) as i64 - (ONE / 2) as i64) * 2 * bend) >> 16;
    let (x, y) = (x as i64 + moved(WARP_INDEX), y as i64 + moved(WARP_INDEX + 1));
    let (square_x, square_y) = (x.div_euclid(side), y.div_euclid(side));
    let mut sites: [Site; 25] = std::array::from_fn(|square| {
        let (square_x, square_y) = (square_x + square as i64 % 5 - 2, square_y + square as i64 / 5 - 2);
        let lot = mix(seed ^ SITES_SALT ^ ((square_x as u32 as u64) << 32 | square_y as u32 as u64).wrapping_mul(GOLDEN_RATIO));
        // In the square's middle half: no site that counts is then past the 25.
        let within = |bits: u64| side / 4 + (((bits & 0xFFFF) as i64 * (side / 2)) >> 16);
        let (across, down) = (x - (square_x * side + within(lot)), y - (square_y * side + within(lot >> 16)));
        Site { away: ((across * across + down * down) as u64).isqrt(), land: (lot >> 32) & 0xFFFF >= polygons.ocean, level: ((lot >> 48) * polygons.levels) >> 16, lot }
    });
    sites.sort_unstable_by_key(|site| site.away);
    sites
}

/// The land at the cell `(x, y)`, in heights: its polygon's level --
/// the lowest ground if it is ocean, over the ocean's level if it is
/// land -- mixed near a border with those of the polygons about it.
pub fn land(shape: &Shape, seed: u64, x: u32, y: u32) -> u64 {
    let sites = sites(shape, seed, x, y);
    // Each site counts by how little farther it is than the nearest: not at all past twice the edge.
    let band = (2 * shape.polygons.edge.max(1)).min(1 << shape.polygons.span.clamp(6, 24));
    let (mut levels, mut counted) = (0, 0);
    for site in sites {
        let counts = band.saturating_sub(site.away - sites[0].away).pow(2);
        let level = if site.land { shape.ocean as u64 + 1 + site.level } else { shape.ground as u64 };
        (levels, counted) = (levels + counts * level, counted + counts);
    }
    levels / counted
}

/// The polygon of the cell `(x, y)`: a number that tells it from
/// every other, whether it is land, and about how many cells the cell
/// is from its border.
pub fn polygon(shape: &Shape, seed: u64, x: u32, y: u32) -> (u64, bool, u64) {
    let sites = sites(shape, seed, x, y);
    (sites[0].lot, sites[0].land, (sites[1].away - sites[0].away) / 2)
}
