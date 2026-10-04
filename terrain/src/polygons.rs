//! The land as polygons: the world cut into closed shapes that share
//! borders and never overlap, each ocean or land, each land one a
//! plain at a level of its own.
//!
//! A polygon is the cells nearer one site than any other. The sites
//! are one to each square of a grid, placed by lot in the square's
//! middle half, so a cell's nearest -- and every one little farther --
//! is among the 25 squares about it ([`Lands`] keeps those from one
//! cell to the next): any cell's polygon follows from the seed and the cell alone.
//! The cell is first moved by broad noise, which bends the borders.
//! Near a border the levels of the polygons about it are mixed, over
//! the edge's width -- half a square's side at most, past which a site
//! beyond the 25 would count: a ramp, a shore, or -- the edge narrow --
//! a cliff.

use crate::{noise, Shape, ONE};
use chunk_storage::Height;
use utilities::hash::{mix, GOLDEN_RATIO};

/// What the sites are drawn by: mixed with the seed, so that they lie apart from all else.
const SITES_SALT: u64 = 0x706F_6C79_676F_6E73;
/// The number the noise that bends the borders is drawn by, across; down is the next.
const WARP_INDEX: u32 = 30;

/// A site.
#[derive(Clone, Copy, Default)]
struct Site {
    /// Where it is, in cells: `(x, y)`.
    at: (i64, i64),
    /// Whether its polygon is land.
    land: bool,
    /// The height its polygon's level is at: the lowest ground if it
    /// is ocean, over the ocean's level if it is land.
    level: u64,
    /// Its lot: what tells it from every other.
    lot: u64,
}

/// The land of a world, asked for cell after cell: the sites about
/// the last cell are kept, for the next is nearly always among the
/// same -- so they are drawn once for a square of the grid, not once
/// for a cell.
pub struct Lands {
    /// How the heights are shaped.
    shape: Shape,
    /// The world's seed.
    seed: u64,
    /// The cells along a square of the grid, as a power of two; and so many.
    side: (u32, i64),
    /// Cells a border is bent by at most.
    bend: i64,
    /// Cells farther than the nearest a site may be and still count.
    band: u64,
    /// The square the sites kept are about, if any are.
    square: Option<(i64, i64)>,
    /// The sites of the 25 squares about it.
    sites: [Site; 25],
}

impl Lands {
    /// The land of the world of `seed`, shaped as `shape` says.
    pub fn new(shape: &Shape, seed: u64) -> Self {
        let span = shape.span.clamp(6, 24);
        let side = 1i64 << span;
        // Each site counts by how little farther it is than the nearest: not at all past twice the edge.
        let band = (2 * shape.edge.max(1)).min(side as u64);
        Self { shape: *shape, seed, side: (span, side), bend: ((side as u64 * shape.warp) >> 16) as i64, band, square: None, sites: [Site::default(); 25] }
    }

    /// The cell `(x, y)` moved by broad noise -- what bends the
    /// borders -- with the sites kept those about where it is moved to.
    fn moved(&mut self, x: u32, y: u32) -> (i64, i64) {
        let (span, side) = self.side;
        let moved = |index: u32| ((noise(self.seed, index, span - 2, x, y) as i64 - (ONE / 2) as i64) * 2 * self.bend) >> 16;
        let (x, y) = (x as i64 + moved(WARP_INDEX), y as i64 + moved(WARP_INDEX + 1));
        let square = (x.div_euclid(side), y.div_euclid(side));
        if self.square != Some(square) {
            self.square = Some(square);
            self.sites = std::array::from_fn(|about| {
                let (square_x, square_y) = (square.0 + about as i64 % 5 - 2, square.1 + about as i64 / 5 - 2);
                let lot = mix(self.seed ^ SITES_SALT ^ ((square_x as u32 as u64) << 32 | square_y as u32 as u64).wrapping_mul(GOLDEN_RATIO));
                // In the square's middle half: no site that counts is then past the 25.
                let within = |bits: u64| side / 4 + (((bits & 0xFFFF) as i64 * (side / 2)) >> 16);
                let land = (lot >> 32) & 0xFFFF >= self.shape.sea;
                // A plain: from just over the ocean to the highest there may be.
                let lowest = self.shape.ocean as u64 + 1;
                let level = if land { lowest + (((lot >> 48) * (self.shape.highest as u64).saturating_sub(lowest)) >> 16) } else { self.shape.ground as u64 };
                Site { at: (square_x * side + within(lot), square_y * side + within(lot >> 16)), land, level, lot }
            });
        }
        (x, y)
    }

    /// The cells from `(x, y)`, moved, to each site kept, squared.
    fn squared(&self, (x, y): (i64, i64)) -> [u64; 25] {
        self.sites.map(|site| ((x - site.at.0).pow(2) + (y - site.at.1).pow(2)) as u64)
    }

    /// The land at the cell `(x, y)`, in heights: its polygon's level,
    /// mixed near a border with those of the polygons about it.
    pub fn land(&mut self, x: u32, y: u32) -> u64 {
        let moved = self.moved(x, y);
        let squared = self.squared(moved);
        let nearest = squared.iter().min().map_or(0, |squared| squared.isqrt());
        // No root taken of a site too far to count: most cells count one alone.
        let reach = (nearest + self.band).pow(2);
        let (mut levels, mut counted) = (0, 0);
        for (site, squared) in self.sites.iter().zip(squared).filter(|&(_, squared)| squared < reach) {
            let counts = (self.band - (squared.isqrt() - nearest)).pow(2);
            (levels, counted) = (levels + counts * site.level, counted + counts);
        }
        levels / counted
    }

    /// The height of the cell `(x, y)`: its land, and no more than the
    /// highest there is, whatever the shape would come to.
    pub fn height(&mut self, x: u32, y: u32) -> Height {
        self.land(x, y).min(Height::MAX as u64) as Height
    }

    /// The polygon of the cell `(x, y)`: a number that tells it from
    /// every other, whether it is land, and about how many cells the
    /// cell is from its border.
    pub fn polygon(&mut self, x: u32, y: u32) -> (u64, bool, u64) {
        let moved = self.moved(x, y);
        let squared = self.squared(moved);
        let nearest = (0..squared.len()).min_by_key(|&site| squared[site]).unwrap_or(0);
        let next = (0..squared.len()).filter(|&site| site != nearest).map(|site| squared[site]).min().unwrap_or(0);
        (self.sites[nearest].lot, self.sites[nearest].land, (next.isqrt() - squared[nearest].isqrt()) / 2)
    }
}

/// The land at the cell `(x, y)`: [`Lands::land`], for one cell alone.
pub fn land(shape: &Shape, seed: u64, x: u32, y: u32) -> u64 {
    Lands::new(shape, seed).land(x, y)
}

/// The polygon of the cell `(x, y)`: [`Lands::polygon`], for one cell alone.
pub fn polygon(shape: &Shape, seed: u64, x: u32, y: u32) -> (u64, bool, u64) {
    Lands::new(shape, seed).polygon(x, y)
}
