//! The land as a mesh: vertices that carry heights, joined by lines
//! that carry how the heights are blended.
//!
//! A vertex is one to each square of a grid, placed by lot in the
//! square's middle half; the four of neighbouring squares make a
//! quad, cut by lot along one diagonal or the other into two
//! triangles. Every cell is in one triangle, found among the eight
//! about its square, so its land follows from the seed and the cell
//! alone. The cell is first moved by broad noise, which bends the lines.
//!
//! A vertex is ocean, at the lowest ground, or land, at a height of
//! its own. A line has a blend, the share of its length the change
//! from one end's height to the other's is spread over -- all of it,
//! and the line is one slope from vertex to vertex -- and a sigmoidness, how much
//! of a step that change is -- each by lot. Along a line the height is
//! its two ends' blended so; within a triangle each vertex's height
//! counts by how near the cell is to it beside the nearer of the
//! others, shaped by its two lines' blend and sigmoidness, each
//! counting as the cell is nearer that line's other end. So about a
//! vertex the ground is a plain; across a line it is a ramp or a
//! cliff; and at a line two triangles agree.
//!
//! Land is low by the ocean and higher inland: a land vertex's height
//! is drawn between just over the ocean and the highest, any height
//! as likely as another past the coast's breadth of vertices from the
//! ocean, but the nearer the ocean the less likely the higher -- so
//! most coasts are low, each by a little of its own, and a few are cliffs.
//!
//! Finer meshes, each with vertices half as far apart as the one
//! before, down to the finest there may be, raise or sink the land by
//! less each: points spread again within the triangles of the mesh
//! before, small variations at a time,
//! and by less the lower the land stands: differences compound inland.

use crate::{noise, Shape, ONE};
use chunk_storage::Height;
use utilities::hash::{mix, GOLDEN_RATIO};

/// What the vertices are drawn by: mixed with the seed, so that they lie apart from all else.
const VERTICES_SALT: u64 = 0x706F_6C79_676F_6E73;
/// The number the noise that bends the lines is drawn by, across; down is the next.
const WARP_INDEX: u32 = 30;
/// The finer meshes there are at most.
pub const FINER_MOST: usize = 10;
/// The cells along a square of the finest grid there may be, as a
/// power of two: no line is much shorter than its 16 cells.
const FINEST: u32 = 4;
/// The vertices the coast is broad at most.
pub const COAST_MOST: u32 = 4;
/// One, of a sigmoidness: 8 bits of fraction.
pub const SIGMOID_ONE: u64 = 1 << 8;
/// The most a sigmoidness is.
const SIGMOID_MOST: u64 = 16 * SIGMOID_ONE;

/// `share`, of [`ONE`], raised to the power `to`, of [`SIGMOID_ONE`]:
/// between two whole powers, as far from one to the other as `to` is.
fn raised(share: u64, to: u64) -> u64 {
    let (whole, part) = (to / SIGMOID_ONE, to % SIGMOID_ONE);
    let lower = (1..whole).fold(share, |raised, _| (raised * share) >> 16);
    let higher = (lower * share) >> 16;
    (lower * (SIGMOID_ONE - part) + higher * part) / SIGMOID_ONE
}

/// A vertex.
#[derive(Clone, Copy, Default)]
struct Vertex {
    /// Where it is, in cells: `(x, y)`.
    at: (i64, i64),
    /// How much the finer meshes vary the land about it, of [`ONE`]:
    /// not at all if it is ocean, a quarter if it is the lowest land,
    /// wholly if the highest. Of a finer mesh, nothing.
    inland: u64,
    /// Its height; of a finer mesh, how far it raises the land, or
    /// under 0 sinks it.
    height: i64,
    /// Its lot: what tells it from every other.
    lot: u64,
}

/// The triangle a cell is in.
#[derive(Clone, Copy)]
struct Triangle {
    /// Its vertices: which of the mesh's kept.
    corners: [usize; 3],
    /// Its lines' blends, each the share of the line's length the
    /// change of height is spread over, of [`ONE`]: the line from each
    /// corner to the next.
    blends: [u64; 3],
    /// Its lines' sigmoidness, of [`SIGMOID_ONE`], likewise.
    sigmoids: [u64; 3],
}

/// What a mesh says of a cell.
struct Blended {
    /// The height there.
    height: i64,
    /// How much the finer meshes vary the land there, of [`ONE`]: not
    /// at all in the ocean.
    inland: u64,
    /// About how many cells the cell is from the nearest line.
    line: u64,
}

/// Twice the area of the triangle `a`, `b`, `c`, signed by which way round it goes.
fn area(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> i64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// A mesh: the vertices of one breadth of grid.
struct Mesh {
    /// How many meshes broader there are: 0 for the land's and the ocean's own.
    depth: u32,
    /// The cells along a square.
    side: i64,
    /// The square the vertices kept are about, if any are.
    square: Option<(i64, i64)>,
    /// The vertices of the nine squares about it, row by row.
    vertices: [Vertex; 9],
    /// The triangle the last cell was in, if it is one of these vertices'.
    last: Option<Triangle>,
}

impl Mesh {
    /// The mesh `depth` meshes finer than the broadest, of `shape`.
    fn new(shape: &Shape, depth: u32) -> Self {
        Self { depth, side: 1i64 << shape.span.clamp(FINEST, 24).saturating_sub(depth).max(FINEST), square: None, vertices: [Vertex::default(); 9], last: None }
    }

    /// The lot of the vertex of the square `(x, y)` of the grid.
    fn lot(&self, seed: u64, (x, y): (i64, i64)) -> u64 {
        mix(seed ^ VERTICES_SALT ^ (self.depth as u64).wrapping_mul(0xA24B_AED4_963E_E407) ^ ((x as u32 as u64) << 32 | y as u32 as u64).wrapping_mul(GOLDEN_RATIO))
    }

    /// Whether the vertex of the square `(x, y)` of the broad grid is
    /// ocean: by its own lot, or -- for a share of them, so that land
    /// and ocean clump a little -- by the lot of the two by two squares
    /// it is one of.
    fn ocean(&self, shape: &Shape, seed: u64, (x, y): (i64, i64)) -> bool {
        let lot = self.lot(seed, (x, y));
        let clumped = (mix(lot) >> 32) & 0xFFFF < shape.clumping;
        let by = if clumped { self.lot(seed ^ VERTICES_SALT, (x.div_euclid(2), y.div_euclid(2))) } else { lot };
        (by >> 32) & 0xFFFF < shape.sea
    }

    /// The vertex of the square `(x, y)` of the grid.
    fn vertex(&self, shape: &Shape, seed: u64, (x, y): (i64, i64)) -> Vertex {
        let lot = self.lot(seed, (x, y));
        // In the square's middle half: the quads are then never folded, and a cell's is among the four about its square.
        let within = |bits: u64| self.side / 4 + (((bits & 0xFFFF) as i64 * (self.side / 2)) >> 16);
        let drawn = (lot >> 32) & 0xFFFF;
        let (inland, height) = if self.depth == 0 {
            if self.ocean(shape, seed, (x, y)) {
                (0, shape.ground as i64)
            } else {
                // How many vertices to the nearest ocean one: one more than the coast is broad if none is so near.
                let breadth = shape.coast.min(COAST_MOST) as i64;
                let ocean = |ring: i64| (-ring..=ring).flat_map(|down| (-ring..=ring).map(move |across| (across, down))).filter(|&(across, down)| across.abs().max(down.abs()) == ring).any(|(across, down)| self.ocean(shape, seed, (x + across, y + down)));
                let away = (1..=breadth).find(|&ring| ocean(ring)).unwrap_or(breadth + 1) as u64;
                // Its share of the way from just over the ocean to the highest: any share as likely as another past the
                // coast, but by the ocean the higher the less likely -- the lot raised to a power, the more the nearer.
                let power = SIGMOID_ONE + if breadth == 0 { 0 } else { shape.coast_low.saturating_sub(SIGMOID_ONE) * (breadth as u64 + 1 - away) / breadth as u64 };
                let share = raised(lot >> 48, power.clamp(SIGMOID_ONE, SIGMOID_MOST));
                let lowest = shape.ocean as u64 + 1;
                // The finer meshes vary the lowest land a quarter as much as the highest: never not at all.
                (ONE / 4 + share * 3 / 4, (lowest + (((shape.highest as u64).saturating_sub(lowest) * share) >> 16)) as i64)
            }
        } else {
            // Some raise or sink the land, by so much less each mesh finer.
            let most = (1..self.depth).fold(shape.finer_height, |most, _| (most * shape.finer_fall) >> 16);
            let by = (((lot >> 48) * most) >> 16) as i64;
            let raised = mix(lot) & 0xFFFF < shape.raised;
            (0, if drawn >= shape.finer_share { 0 } else if raised { by } else { -by })
        };
        Vertex { at: (x * self.side + within(lot), y * self.side + within(lot >> 16)), inland, height, lot }
    }

    /// The triangle of `corners`, its lines' blends and sigmoidness drawn.
    fn triangle(&self, shape: &Shape, corners: [usize; 3]) -> Triangle {
        let line = |from: usize| {
            // By its two ends, whichever is named first: the same seen from either triangle.
            let lot = mix(self.vertices[corners[from]].lot ^ self.vertices[corners[(from + 1) % 3]].lot);
            let blend = (shape.narrow + (((lot & 0xFFFF) * shape.wide.saturating_sub(shape.narrow)) >> 16)).clamp(1, ONE);
            (blend, shape.soft + ((((lot >> 16) & 0xFFFF) * shape.hard.saturating_sub(shape.soft)) >> 16))
        };
        let lines = [line(0), line(1), line(2)];
        Triangle { corners, blends: lines.map(|line| line.0), sigmoids: lines.map(|line| line.1.clamp(SIGMOID_ONE, SIGMOID_MOST)) }
    }

    /// The triangle the cell `at`, moved, is in, and how near each of
    /// its corners the cell is, of [`ONE`] and to [`ONE`] in all.
    fn locate(&mut self, shape: &Shape, seed: u64, at: (i64, i64)) -> (Triangle, [u64; 3]) {
        let square = (at.0.div_euclid(self.side), at.1.div_euclid(self.side));
        if self.square != Some(square) {
            (self.square, self.last) = (Some(square), None);
            self.vertices = std::array::from_fn(|about| self.vertex(shape, seed, (square.0 + about as i64 % 3 - 1, square.1 + about as i64 / 3 - 1)));
        }
        // How near each corner, if the cell is within: each corner's share of the triangle's area.
        let near = |corners: [usize; 3], strictly: bool| {
            let [a, b, c] = corners.map(|corner| self.vertices[corner].at);
            let (whole, first, second) = (area(a, b, c), area(at, b, c), area(a, at, c));
            let third = whole - first - second;
            // On a line, a cell is in both its triangles: only the first looked at in a fixed order may claim it, never the one kept.
            let inside = |strictly: bool| whole != 0 && [first, second, third].iter().all(|&part| (part == 0 && !strictly) || (part != 0 && (part > 0) == (whole > 0)));
            inside(strictly).then(|| {
                // In 64 bits wherever that is room enough: all but the broadest meshes.
                let share = |part: i64| if part.unsigned_abs() >> 47 == 0 { (part.unsigned_abs() << 16) / whole.unsigned_abs() } else { ((part.unsigned_abs() as u128 * ONE as u128) / whole.unsigned_abs() as u128) as u64 };
                let (first, second) = (share(first), share(second));
                [first, second, ONE.saturating_sub(first + second)]
            })
        };
        // The last cell's triangle, as likely as not.
        if let Some((triangle, near)) = self.last.and_then(|last| Some((last, near(last.corners, true)?))) {
            return (triangle, near);
        }
        for quad in 0..4 {
            let corner = quad % 2 + quad / 2 * 3;
            let (here, east, south, far) = (corner, corner + 1, corner + 3, corner + 4);
            // Cut along one diagonal or the other, by the quad's first vertex's lot.
            let halves = if self.vertices[here].lot >> 63 == 0 { [[here, east, far], [here, far, south]] } else { [[here, east, south], [east, far, south]] };
            for corners in halves {
                if let Some(near) = near(corners, false) {
                    let triangle = self.triangle(shape, corners);
                    self.last = Some(triangle);
                    return (triangle, near);
                }
            }
        }
        // In none, by a rounding: at the square's own vertex.
        (Triangle { corners: [4; 3], blends: [ONE; 3], sigmoids: [SIGMOID_ONE; 3] }, [ONE, 0, 0])
    }

    /// What the mesh says of the cell `at`, moved.
    fn blended(&mut self, shape: &Shape, seed: u64, at: (i64, i64)) -> Blended {
        let (triangle, near) = self.locate(shape, seed, at);
        let (mut heights, mut inlands, mut counted) = (0i64, 0u64, 0u64);
        for corner in 0..3 {
            let (next, other) = ((corner + 1) % 3, (corner + 2) % 3);
            // Its two lines' blend and sigmoidness, each counting as the cell is nearer that line's other end.
            let both = (near[next] + near[other]).max(1);
            let mixed = |of: &[u64; 3]| if near[next] + near[other] == 0 { of[corner] } else { (of[corner] * near[next] + of[other] * near[other]) / both };
            let (blend, sigmoid) = (mixed(&triangle.blends).max(1), mixed(&triangle.sigmoids));
            // How near this corner beside the nearer of the others: a half where the two are as near.
            let beside = (near[corner] << 16) / (near[corner] + near[next].max(near[other])).max(1);
            // The change is all within the blend, about the half.
            let along = (((beside as i64 - (ONE / 2) as i64 + (blend / 2) as i64) << 16) / blend as i64).clamp(0, ONE as i64) as u64;
            let (towards, from) = (raised(along, sigmoid), raised(ONE - along, sigmoid));
            let counts = (towards << 16) / (towards + from).max(1);
            let vertex = &self.vertices[triangle.corners[corner]];
            (heights, inlands, counted) = (heights + counts as i64 * vertex.height, inlands + counts * vertex.inland, counted + counts);
        }
        let counted = counted.max(1);
        Blended { height: heights / counted as i64, inland: inlands / counted, line: (near.iter().min().copied().unwrap_or(0) * self.side as u64) >> 16 }
    }
}

/// The land of a world, asked for cell after cell: the vertices about
/// the last cell and its triangle are kept, for the next is nearly
/// always in the same -- so they are drawn once for a square of a
/// grid, not once for a cell.
pub struct Lands {
    /// How the heights are shaped.
    shape: Shape,
    /// The world's seed.
    seed: u64,
    /// The cells along a square of the broadest grid, as a power of two.
    span: u32,
    /// Cells a line is bent by at most.
    bend: i64,
    /// The land's and the ocean's mesh.
    broad: Mesh,
    /// The finer meshes, the broadest first.
    finer: [Mesh; FINER_MOST],
}

impl Lands {
    /// The land of the world of `seed`, shaped as `shape` says.
    pub fn new(shape: &Shape, seed: u64) -> Self {
        let span = shape.span.clamp(FINEST, 24);
        Self { shape: *shape, seed, span, bend: (((1u64 << span) * shape.warp) >> 16) as i64, broad: Mesh::new(shape, 0), finer: std::array::from_fn(|finer| Mesh::new(shape, finer as u32 + 1)) }
    }

    /// The cell `(x, y)` moved by broad noise: what bends the lines.
    fn moved(&self, x: u32, y: u32) -> (i64, i64) {
        let moved = |index: u32| ((noise(self.seed, index, self.span - 2, x, y) as i64 - (ONE / 2) as i64) * 2 * self.bend) >> 16;
        (x as i64 + moved(WARP_INDEX), y as i64 + moved(WARP_INDEX + 1))
    }

    /// The land at the cell `(x, y)`, in heights: the broad mesh's
    /// height there, and what the finer meshes raise or sink it by, as
    /// much of it as the land there stands high -- never under the lowest ground.
    pub fn land(&mut self, x: u32, y: u32) -> u64 {
        let at = self.moved(x, y);
        let broad = self.broad.blended(&self.shape, self.seed, at);
        if broad.inland == 0 {
            return broad.height.max(self.shape.ground as i64) as u64;
        }
        // No finer than the finest, and none that would move the land by less than a height: those are left out.
        let moving = (0..FINER_MOST).take_while(|&finer| (0..finer).fold(self.shape.finer_height, |most, _| (most * self.shape.finer_fall) >> 16) > 0).count();
        let depth = (self.shape.finer_depth as usize).min(moving).min((self.span - FINEST) as usize);
        let finer: i64 = self.finer.iter_mut().take(depth).map(|mesh| mesh.blended(&self.shape, self.seed, at).height).sum();
        (broad.height + ((finer * broad.inland as i64) >> 16)).max(self.shape.ground as i64) as u64
    }

    /// The height of the cell `(x, y)`: its land, and no more than the
    /// highest there is, whatever the shape would come to.
    pub fn height(&mut self, x: u32, y: u32) -> Height {
        self.land(x, y).min(Height::MAX as u64) as Height
    }

    /// How high the land at the cell `(x, y)` stands of what it may, of
    /// [`ONE`], and about
    /// how many cells it is from the nearest line of the broad mesh.
    pub fn line(&mut self, x: u32, y: u32) -> (u64, u64) {
        let at = self.moved(x, y);
        let broad = self.broad.blended(&self.shape, self.seed, at);
        (broad.inland, broad.line)
    }
}

/// The land at the cell `(x, y)`: [`Lands::land`], for one cell alone.
pub fn land(shape: &Shape, seed: u64, x: u32, y: u32) -> u64 {
    Lands::new(shape, seed).land(x, y)
}
