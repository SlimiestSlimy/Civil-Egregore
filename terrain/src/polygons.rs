//! The land as polygons: the world cut into closed shapes that share
//! borders and never overlap, each ocean or land, each land one a
//! plain at a level of its own; within the land ones, smaller
//! polygons, plains raised or sunk; and lines, ridges and canyons.
//!
//! A polygon is the cells nearer one site than any other. The sites
//! are one to each square of a grid, placed by lot in the square's
//! middle half, so a cell's nearest -- and every one little farther --
//! is among the 25 squares about it ([`Lands`] keeps those from one
//! cell to the next): any cell's land follows from the seed and the
//! cell alone. The cell is first moved by broad noise, which bends the
//! borders.
//!
//! Near a border the levels of the polygons about it are mixed, over
//! a ramp. Each polygon has a ramp's width of its own, by lot: about
//! a narrow one the ground meets its neighbours' by cliffs -- a mesa,
//! a sunk plain with walls -- about a broad one by slopes. The width
//! at a cell is mixed from those of the polygons about it as the
//! levels are, the narrow counting most, so it too has no jump. And
//! each has a hardness of its own, by lot: soft, and the ramp is an
//! even slope from one level to the other; hard, and it is two levels
//! and a step between them -- the more of a sigmoid the harder.
//!
//! Smaller polygons lie within the land ones, on grids each a quarter
//! the breadth of the one before: some of them, by lot, raise or sink
//! the ground by a level of their own. And each land polygon has a few
//! lines by lot, each a chain of segments about its site: a ridge or a
//! canyon, highest or deepest on the line and gone a width from it.
//! Both fade to nothing at their polygon's border, so they stay within it.

use crate::{noise, Shape, ONE};
use chunk_storage::Height;
use utilities::hash::{mix, GOLDEN_RATIO};

/// What the sites are drawn by: mixed with the seed, so that they lie apart from all else.
const SITES_SALT: u64 = 0x706F_6C79_676F_6E73;
/// The number the noise that bends the borders is drawn by, across; down is the next.
const WARP_INDEX: u32 = 30;
/// The grids of smaller polygons there are at most.
pub const INNER_MOST: usize = 3;
/// The lines a polygon has at most.
pub const LINES_MOST: usize = 4;
/// The points a line's chain of segments goes through.
const JOINTS: usize = 4;
/// Cells from its polygon's border over which what lies within it fades, at most.
const FADE: u64 = 512;

/// One, of a ramp's hardness: 8 bits of fraction.
pub const HARD_ONE: u64 = 1 << 8;
/// The hardest a ramp is.
const HARDEST: u64 = 16 * HARD_ONE;

/// `nearness`, of [`ONE`], raised to the power `hard`, of [`HARD_ONE`]:
/// between two whole powers, as far from one to the other as `hard` is.
fn raised(nearness: u64, hard: u64) -> u64 {
    let (whole, part) = (hard / HARD_ONE, hard % HARD_ONE);
    let lower = (1..whole).fold(nearness, |raised, _| (raised * nearness) >> 16);
    let higher = (lower * nearness) >> 16;
    (lower * (HARD_ONE - part) + higher * part) / HARD_ONE
}

/// A width drawn by `lot` between `narrow` and `wide`, as likely
/// narrow as wide: `narrow` doubled some number of times.
fn width(lot: u64, narrow: u64, wide: u64) -> u64 {
    let (narrow, wide) = (narrow.max(1), wide.max(narrow.max(1)));
    (narrow << (lot % ((wide / narrow).ilog2() as u64 + 1))).min(wide)
}

/// A site.
#[derive(Clone, Copy, Default)]
struct Site {
    /// Where it is, in cells: `(x, y)`.
    at: (i64, i64),
    /// Whether its polygon is land; of a smaller polygon, whether it
    /// raises or sinks the ground at all.
    land: bool,
    /// The height its polygon's level is at; of a smaller polygon, how
    /// far it raises the ground, or under 0 sinks it.
    level: i64,
    /// The cells its ramp is across.
    ramp: u64,
    /// How hard its ramp is, of [`HARD_ONE`]: the power a site's
    /// nearness is raised to before it counts.
    hard: u64,
    /// Its lot: what tells it from every other.
    lot: u64,
}

/// What a grid says of a cell.
struct Mixed {
    /// The level there: the nearest polygon's, mixed near a border
    /// with those about it.
    level: i64,
    /// Which of the grid's sites kept is nearest.
    nearest: usize,
    /// About how many cells the cell is from its polygon's border.
    border: u64,
    /// The cells a ramp is across there.
    ramp: u64,
}

/// A grid of sites: the polygons of one breadth.
struct Grid {
    /// How many grids broader there are: 0 for the land's and the ocean's own.
    depth: u32,
    /// The cells along a square.
    side: i64,
    /// Cells farther than the nearest a site may be and still count
    /// towards the ramp's width.
    band: u64,
    /// The square the sites kept are about, if any are.
    square: Option<(i64, i64)>,
    /// The sites of the 25 squares about it.
    sites: [Site; 25],
}

impl Grid {
    /// The grid `depth` grids finer than the broadest, of `shape`.
    fn new(shape: &Shape, depth: u32) -> Self {
        let side = 1i64 << shape.span.clamp(6, 24).saturating_sub(2 * depth).max(6);
        Self { depth, side, band: (2 * shape.wide.max(1)).min(side as u64), square: None, sites: [Site::default(); 25] }
    }

    /// The site of the square `(x, y)` of the grid.
    fn site(&self, shape: &Shape, seed: u64, (x, y): (i64, i64)) -> Site {
        let lot = mix(seed ^ SITES_SALT ^ (self.depth as u64).wrapping_mul(0xA24B_AED4_963E_E407) ^ ((x as u32 as u64) << 32 | y as u32 as u64).wrapping_mul(GOLDEN_RATIO));
        let more = mix(lot);
        // In the square's middle half: no site that counts is then past the 25.
        let within = |bits: u64| self.side / 4 + (((bits & 0xFFFF) as i64 * (self.side / 2)) >> 16);
        let drawn = (lot >> 32) & 0xFFFF;
        let (land, level) = if self.depth == 0 {
            // A plain, from just over the ocean to the highest there may be; or the ocean's floor.
            let lowest = shape.ocean as u64 + 1;
            let land = drawn >= shape.sea;
            (land, if land { lowest + (((lot >> 48) * (shape.highest as u64).saturating_sub(lowest)) >> 16) } else { shape.ground as u64 } as i64)
        } else {
            // Some raise or sink the ground, less the finer the grid.
            let by = (((lot >> 48) * (shape.inner_height >> (self.depth - 1))) >> 16) as i64;
            let raised = (more >> 8) & 0xFFFF < shape.raised;
            (drawn < shape.inner_share, if drawn >= shape.inner_share { 0 } else if raised { by } else { -by })
        };
        Site { at: (x * self.side + within(lot), y * self.side + within(lot >> 16)), land, level, ramp: width(more & 0xFF, shape.narrow, shape.wide), hard: shape.soft + ((((more >> 24) & 0xFFFF) * shape.hard.saturating_sub(shape.soft)) >> 16), lot }
    }

    /// Keeps the sites about the cell `at`, moved: drawn again only
    /// when it is in another square than the last.
    fn about(&mut self, shape: &Shape, seed: u64, at: (i64, i64)) {
        let square = (at.0.div_euclid(self.side), at.1.div_euclid(self.side));
        if self.square != Some(square) {
            self.square = Some(square);
            self.sites = std::array::from_fn(|about| self.site(shape, seed, (square.0 + about as i64 % 5 - 2, square.1 + about as i64 / 5 - 2)));
        }
    }

    /// What the grid says of the cell `(x, y)`, moved, the sites kept
    /// being those about it.
    fn mixed(&self, (x, y): (i64, i64)) -> Mixed {
        let squared = self.sites.map(|site| ((x - site.at.0).pow(2) + (y - site.at.1).pow(2)) as u64);
        let nearest = (0..squared.len()).min_by_key(|&site| squared[site]).unwrap_or(0);
        let next = (0..squared.len()).filter(|&site| site != nearest).map(|site| squared[site]).min().unwrap_or(0);
        let near = squared[nearest].isqrt();
        // How much farther than the nearest each is: no root taken of one too far to count.
        let reach = (near + self.band).pow(2);
        let farther = squared.map(|squared| if squared < reach { squared.isqrt() - near } else { u64::MAX });
        // The ramp's width and hardness here: those about, each counting by how little farther its site is, the narrow most.
        let (mut narrowness, mut hardness, mut counted) = (0u128, 0u128, 0u128);
        for (site, &farther) in self.sites.iter().zip(&farther).filter(|&(_, &farther)| farther < self.band) {
            let counts = ((self.band - farther) as u128).pow(2);
            (narrowness, hardness, counted) = (narrowness + counts * ((1u128 << 32) / site.ramp.max(1) as u128), hardness + counts * site.hard as u128, counted + counts);
        }
        let ramp = (((1u128 << 32) / (narrowness / counted).max(1)) as u64).clamp(1, self.band);
        let hard = ((hardness / counted) as u64).clamp(HARD_ONE, HARDEST);
        // The level: those about, each counting by how near the nearest its site is -- the harder the ramp, the less -- none past the ramp.
        let (mut levels, mut counted) = (0i128, 0i128);
        for (site, &farther) in self.sites.iter().zip(&farther).filter(|&(_, &farther)| farther < ramp) {
            let counts = raised(((ramp - farther) << 16) / ramp, hard) as i128;
            (levels, counted) = (levels + counts * site.level as i128, counted + counts);
        }
        Mixed { level: (levels / counted) as i64, nearest, border: (next.isqrt() - near) / 2, ramp }
    }
}

/// A line: a ridge or a canyon.
#[derive(Clone, Copy, Default)]
struct Line {
    /// The points its chain of segments goes through, in cells.
    joints: [(i64, i64); JOINTS],
    /// How far it raises the ground on the line, or under 0 sinks it.
    height: i64,
    /// The cells from the line it is gone at.
    width: u64,
}

impl Line {
    /// How far the line raises or sinks the ground at the cell `at`, moved.
    fn at(&self, at: (i64, i64)) -> i64 {
        let away = self.joints.windows(2).map(|ends| away(at, ends[0], ends[1])).min().unwrap_or(u64::MAX);
        if away >= self.width {
            return 0;
        }
        (self.height as i128 * ((self.width - away) as i128).pow(2) / (self.width as i128).pow(2)) as i64
    }
}

/// Cells from `at` to the segment from `from` to `to`.
fn away(at: (i64, i64), from: (i64, i64), to: (i64, i64)) -> u64 {
    let (along, towards) = ((to.0 - from.0, to.1 - from.1), (at.0 - from.0, at.1 - from.1));
    let length = (along.0 as i128).pow(2) + (along.1 as i128).pow(2);
    // How far along the segment the point of it nearest `at` is, of its length squared.
    let reached = (towards.0 as i128 * along.0 as i128 + towards.1 as i128 * along.1 as i128).clamp(0, length);
    let nearest = |from: i64, along: i64| if length == 0 { from } else { from + (along as i128 * reached / length) as i64 };
    (((at.0 - nearest(from.0, along.0)) as i128).pow(2) + ((at.1 - nearest(from.1, along.1)) as i128).pow(2)).isqrt() as u64
}

/// The land of a world, asked for cell after cell: the sites about
/// the last cell are kept, for the next is nearly always among the
/// same -- so they are drawn once for a square of a grid, not once
/// for a cell -- and so are the lines of the last cell's polygon.
pub struct Lands {
    /// How the heights are shaped.
    shape: Shape,
    /// The world's seed.
    seed: u64,
    /// The cells along a square of the broadest grid, as a power of two.
    span: u32,
    /// Cells a border is bent by at most.
    bend: i64,
    /// The land's and the ocean's polygons.
    outer: Grid,
    /// The smaller polygons within them, the broadest first.
    inner: [Grid; INNER_MOST],
    /// The polygon whose lines are kept, by its lot, if any are.
    lined: Option<u64>,
    /// Its lines: as many as it has, the rest of no height.
    lines: [Line; LINES_MOST],
}

impl Lands {
    /// The land of the world of `seed`, shaped as `shape` says.
    pub fn new(shape: &Shape, seed: u64) -> Self {
        let span = shape.span.clamp(6, 24);
        let bend = (((1u64 << span) * shape.warp) >> 16) as i64;
        Self { shape: *shape, seed, span, bend, outer: Grid::new(shape, 0), inner: std::array::from_fn(|finer| Grid::new(shape, finer as u32 + 1)), lined: None, lines: [Line::default(); LINES_MOST] }
    }

    /// The cell `(x, y)` moved by broad noise: what bends the borders.
    fn moved(&self, x: u32, y: u32) -> (i64, i64) {
        let moved = |index: u32| ((noise(self.seed, index, self.span - 2, x, y) as i64 - (ONE / 2) as i64) * 2 * self.bend) >> 16;
        (x as i64 + moved(WARP_INDEX), y as i64 + moved(WARP_INDEX + 1))
    }

    /// Keeps the lines of the polygon of `site`: drawn again only when
    /// it is another polygon than the last.
    fn line(&mut self, site: &Site) {
        if self.lined == Some(site.lot) {
            return;
        }
        self.lined = Some(site.lot);
        let shape = &self.shape;
        let lines = mix(site.lot ^ SITES_SALT) % (shape.lines.min(LINES_MOST as u32) as u64 + 1);
        // About the site, as far as most of the way to its polygon's border.
        let reach = self.outer.side * 2 / 5;
        self.lines = std::array::from_fn(|line| {
            if line as u64 >= lines {
                return Line::default();
            }
            let lot = mix(site.lot ^ (line as u64 + 1).wrapping_mul(GOLDEN_RATIO));
            let joints = std::array::from_fn(|joint| {
                let lot = mix(lot ^ (joint as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407));
                let about = |bits: u64| (((bits & 0xFFFF) as i64 - (ONE / 2) as i64) * reach) >> 15;
                (site.at.0 + about(lot), site.at.1 + about(lot >> 16))
            });
            let height = (((lot >> 48) * shape.line_height) >> 16) as i64;
            Line { joints, height: if (lot >> 32) & 0xFFFF < shape.ridges { height } else { -height }, width: width(lot & 0xFF, shape.line_narrow, shape.line_wide) }
        });
    }

    /// The land at the cell `(x, y)`, in heights: its polygon's level,
    /// mixed near a border with those of the polygons about it; and
    /// within a land polygon, what its smaller polygons and its lines
    /// raise or sink it by -- never under the lowest ground.
    pub fn land(&mut self, x: u32, y: u32) -> u64 {
        let at = self.moved(x, y);
        self.outer.about(&self.shape, self.seed, at);
        let mixed = self.outer.mixed(at);
        let site = self.outer.sites[mixed.nearest];
        // What lies within a polygon is gone at its border.
        let within = ((mixed.border << 16) / mixed.ramp.clamp(1, FADE)).min(ONE) as i64;
        if !site.land || within == 0 {
            return mixed.level.max(self.shape.ground as i64) as u64;
        }
        let mut moved = 0;
        for finer in 0..(self.shape.inner_depth as usize).min(INNER_MOST) {
            self.inner[finer].about(&self.shape, self.seed, at);
            moved += self.inner[finer].mixed(at).level;
        }
        self.line(&site);
        moved += self.lines.iter().map(|line| line.at(at)).sum::<i64>();
        (mixed.level + ((moved * within) >> 16)).max(self.shape.ground as i64) as u64
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
        let at = self.moved(x, y);
        self.outer.about(&self.shape, self.seed, at);
        let mixed = self.outer.mixed(at);
        (self.outer.sites[mixed.nearest].lot, self.outer.sites[mixed.nearest].land, mixed.border)
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
