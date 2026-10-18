//! The land as a mesh: vertices that carry heights, joined by lines
//! that carry how the heights are blended; finer meshes on it, each
//! moving the land by less (`docs/worldgen.md`, "The land as a mesh").

mod lands;
pub use lands::{WorldLand, land_of_one_cell_cartesian};

use crate::worldgen::{noise_at_cell_cartesian, LandShape, FRACTION_ONE};
use crate::worldgen::lots::{mix_word, GOLDEN_RATIO};

/// What the vertices are drawn by: mixed with the seed, so that they lie apart from all else.
const VERTICES_SALT: u64 = 0x706F_6C79_676F_6E73;
/// The number the noise that bends the lines is drawn by, across; down is the next.
const WARP_INDEX: u32 = 30;
/// The number the noise land and ocean clump by is drawn by; its finer octave is the next.
const CLUMP_INDEX: u32 = 40;
/// The finer meshes there are at most.
pub const FINER_MOST: usize = 10;
/// The cells along a square of the finest grid there may be, as a
/// power of two: no line is much shorter than its 16 cells.
const FINEST_SPACING_POWER: u32 = 4;
/// The vertices the coast is broad at most.
pub const COAST_MOST: u32 = 4;
/// One, of a sigmoidness: 8 bits of fraction.
pub const SIGMOID_ONE: u64 = 1 << 8;
/// The most a sigmoidness is.
const SIGMOID_MOST: u64 = 16 * SIGMOID_ONE;

/// `base_share`, of [`ONE`], raised to the power `power_fraction`, of [`SIGMOID_ONE`]:
/// between two whole powers, as far from one to the other as `power_fraction` is.
fn raised_to_power(base_share: u64, power_fraction: u64) -> u64 {
    let (whole_power, part) = (power_fraction / SIGMOID_ONE, power_fraction % SIGMOID_ONE);
    let lower_raised = (1..whole_power).fold(base_share, |raised_so_far, _| (raised_so_far * base_share) >> 16);
    let higher_raised = (lower_raised * base_share) >> 16;
    (lower_raised * (SIGMOID_ONE - part) + higher_raised * part) / SIGMOID_ONE
}

/// A line's lot, of [`ONE`], drawn a little nearer nothing: halfway
/// from the lot to its square. A line is by so much likelier a slope
/// than a step, its blend wide than narrow -- more land that rises,
/// less that lies level (`docs/worldgen.md`, "The land as a mesh").
fn towards_the_slope(line_lot: u64) -> u64 {
    (line_lot + ((line_lot * line_lot) >> 16)) / 2
}

/// A vertex.
#[derive(Clone, Copy, Default)]
struct Vertex {
    /// Where it is, in cells: `(x, y)`.
    vertex_cartesian: (i64, i64),
    /// How much the finer meshes vary the land about it, of [`ONE`]:
    /// not at all if it is ocean, a quarter if it is the lowest land,
    /// wholly if the highest. Of a finer mesh, nothing.
    inland_share: u64,
    /// Its height; of a finer mesh, how far it raises the land, or
    /// under 0 sinks it.
    height: i64,
    /// Its share of the weight that reached it, of [`ONE`], by lot: what
    /// a finer mesh's vertex moves the land with, and what any vertex
    /// hands on to the meshes finer than its own.
    free_weight: u64,
    /// Its lot: what tells it from every other.
    vertex_lot: u64,
}

/// The triangle a cell is in.
#[derive(Clone, Copy)]
struct MeshTriangle {
    /// Its vertices: which of the mesh's kept.
    corner_vertices: [usize; 3],
    /// Its lines' blends, each the share of the line's length the
    /// change of height is spread over, of [`ONE`]: the line from each
    /// corner to the next.
    blends: [u64; 3],
    /// Its lines' sigmoidness, of [`SIGMOID_ONE`], likewise.
    line_sigmoidness: [u64; 3],
    /// What a part of its area is multiplied by to be its share of the
    /// whole ([`area_inverse`]).
    area_inverse: u64,
}

/// What a mesh says of a cell.
struct BlendedCell {
    /// The height there.
    height: i64,
    /// How much the finer meshes vary the land there, of [`ONE`]: not
    /// at all in the ocean.
    inland_share: u64,
    /// How much the meshes finer may move the land there, of [`ONE`].
    free_weight: u64,
    /// About how many cells the cell is from the nearest line.
    line: u64,
}

/// Twice the area of the triangle `first_point`, `second_point`, `third_point`, signed by which way round it goes.
fn area(first_point: (i64, i64), second_point: (i64, i64), third_point: (i64, i64)) -> i64 {
    (second_point.0 - first_point.0) * (third_point.1 - first_point.1) - (second_point.1 - first_point.1) * (third_point.0 - first_point.0)
}

/// What a part of a triangle's area is multiplied by, and then halved
/// 46 times, to be its share of `whole_area`, of [`ONE`] -- a division done
/// once for the triangle, not thrice for a cell; 0 where the triangle
/// is too broad for that to be exact enough, and the division is done.
fn inverse_of_area(whole_area: i64) -> u64 {
    if whole_area.unsigned_abs() >> 40 == 0 { (1 << 62) / whole_area.unsigned_abs().max(1) } else { 0 }
}

/// A mesh: the vertices of one breadth of grid.
struct Mesh {
    /// How many meshes broader there are: 0 for the land's and the ocean's own.
    mesh_depth: u32,
    /// The cells along a square.
    square_side: i64,
    /// The square the vertices kept are about, if any are.
    kept_square: Option<(i64, i64)>,
    /// The vertices of the nine squares about it, row by row.
    kept_vertices: [Vertex; 9],
    /// The triangle the last cell was in, if it is one of these vertices'.
    last_triangle: Option<MeshTriangle>,
}

impl Mesh {
    /// The mesh `mesh_depth` meshes finer than the broadest, of `land_shape`.
    fn new(land_shape: &LandShape, mesh_depth: u32) -> Self {
        Self { mesh_depth, square_side: 1i64 << land_shape.vertex_spacing_power.clamp(FINEST_SPACING_POWER, 24).saturating_sub(mesh_depth).max(FINEST_SPACING_POWER), kept_square: None, kept_vertices: [Vertex::default(); 9], last_triangle: None }
    }

    /// The lot of the vertex of the square `(x, y)` of the grid.
    fn vertex_lot_cartesian(&self, seed: u64, (x, y): (i64, i64)) -> u64 {
        mix_word(seed ^ VERTICES_SALT ^ (self.mesh_depth as u64).wrapping_mul(0xA24B_AED4_963E_E407) ^ ((x as u32 as u64) << 32 | y as u32 as u64).wrapping_mul(GOLDEN_RATIO))
    }

    /// Whether the vertex of the square `(x, y)` of the broad grid is
    /// ocean: by its own lot, mixed -- so that land and ocean clump --
    /// with smooth noise some vertices broad, which has no blocks to
    /// show through.
    fn ocean(&self, land_shape: &LandShape, seed: u64, (x, y): (i64, i64)) -> bool {
        let own_lot = (self.vertex_lot_cartesian(seed, (x, y)) >> 32) & 0xFFFF;
        if land_shape.land_clumping == 0 {
            return own_lot < land_shape.ocean_share;
        }
        // Two octaves, four vertices and two between points; spread about the half, for noise is mostly near it.
        let (square_across, square_down) = (x as u32, y as u32);
        let clump_noise = (2 * noise_at_cell_cartesian(seed, CLUMP_INDEX, 2, square_across, square_down) + noise_at_cell_cartesian(seed, CLUMP_INDEX + 1, 1, square_across, square_down)) / 3;
        let spread_clump = ((clump_noise as i64 - (FRACTION_ONE / 2) as i64) * 2 + (FRACTION_ONE / 2) as i64).clamp(0, FRACTION_ONE as i64 - 1) as u64;
        (own_lot * (FRACTION_ONE - land_shape.land_clumping) + spread_clump * land_shape.land_clumping) >> 16 < land_shape.ocean_share
    }

    /// The vertex of the square `(x, y)` of the grid.
    fn vertex(&self, land_shape: &LandShape, seed: u64, (x, y): (i64, i64)) -> Vertex {
        let vertex_lot = self.vertex_lot_cartesian(seed, (x, y));
        // In the square's middle half: the quads are then never folded, and a cell's is among the four about its square.
        let offset_within = |lot_bits: u64| self.square_side / 4 + (((lot_bits & 0xFFFF) as i64 * (self.square_side / 2)) >> 16);
        let drawn_share = (vertex_lot >> 32) & 0xFFFF;
        let (inland_share, height) = if self.mesh_depth == 0 {
            if self.ocean(land_shape, seed, (x, y)) {
                (0, land_shape.ocean_floor as i64)
            } else {
                // How many vertices to the nearest ocean one: one more than the coast is broad if none is so near.
                let coast_vertices = land_shape.coast_breadth.min(COAST_MOST) as i64;
                let ocean = |ring_radius: i64| (-ring_radius..=ring_radius).flat_map(|ring_down| (-ring_radius..=ring_radius).map(move |ring_across| (ring_across, ring_down))).filter(|&(ring_across, ring_down)| ring_across.abs().max(ring_down.abs()) == ring_radius).any(|(ring_across, ring_down)| self.ocean(land_shape, seed, (x + ring_across, y + ring_down)));
                let vertices_away = (1..=coast_vertices).find(|&ring_radius| ocean(ring_radius)).unwrap_or(coast_vertices + 1) as u64;
                // Its share of the way from just over the ocean to the highest: any share as likely as another past the
                // coast, but by the ocean the higher the less likely -- the lot raised to a power, the more the nearer.
                let coast_power = SIGMOID_ONE + if coast_vertices == 0 { 0 } else { land_shape.coast_low.saturating_sub(SIGMOID_ONE) * (coast_vertices as u64 + 1 - vertices_away) / coast_vertices as u64 };
                let height_share = raised_to_power(vertex_lot >> 48, coast_power.clamp(SIGMOID_ONE, SIGMOID_MOST));
                let lowest_land = land_shape.ocean as u64 + 1;
                // The finer meshes vary the lowest land a quarter as much as the highest: never not at all.
                (FRACTION_ONE / 4 + height_share * 3 / 4, (lowest_land + (((land_shape.highest_land as u64).saturating_sub(lowest_land) * height_share) >> 16)) as i64)
            }
        } else {
            // Some raise or sink the land, by so much less each mesh finer.
            // Never by less than a height or two: every mesh counts.
            let most_move = (1..self.mesh_depth).fold(land_shape.finer_height, |most_so_far, _| (most_so_far * land_shape.finer_fall) >> 16).max(2.min(land_shape.finer_height));
            let raise_by = (((vertex_lot >> 48) * most_move) >> 16) as i64;
            let is_raised = mix_word(vertex_lot) & 0xFFFF < land_shape.raised_share;
            (0, if drawn_share >= land_shape.finer_share { 0 } else if is_raised { raise_by } else { -raise_by })
        };
        // Its share of the weight that reached it, a byte by lot: all of it, less up to what the shape says. A
        // vertex of a finer mesh moves the land by so much of its height, and hands as much on to its own
        // subdivisions; one of the broad mesh, whose weight is one, only hands it on.
        let free_weight = FRACTION_ONE - (mix_word(vertex_lot ^ VERTICES_SALT) >> 56) * land_shape.weight / 255;
        let height = if self.mesh_depth == 0 { height } else { (height * free_weight as i64) >> 16 };
        Vertex { vertex_cartesian: (x * self.square_side + offset_within(vertex_lot), y * self.square_side + offset_within(vertex_lot >> 16)), inland_share, free_weight, height, vertex_lot }
    }

    /// The triangle of `corner_vertices`, its lines' blends and sigmoidness drawn.
    fn triangle_of_corners(&self, land_shape: &LandShape, corner_vertices: [usize; 3], area_inverse: u64) -> MeshTriangle {
        let line = |from_corner: usize| {
            // By its two ends, whichever is named first: the same seen from either triangle.
            let line_lot = mix_word(self.kept_vertices[corner_vertices[from_corner]].vertex_lot ^ self.kept_vertices[corner_vertices[(from_corner + 1) % 3]].vertex_lot);
            let blend = (land_shape.narrowest_blend + (((FRACTION_ONE - towards_the_slope(FRACTION_ONE - (line_lot & 0xFFFF))) * land_shape.widest_blend.saturating_sub(land_shape.narrowest_blend)) >> 16)).clamp(1, FRACTION_ONE);
            (blend, land_shape.least_sigmoidness + ((towards_the_slope((line_lot >> 16) & 0xFFFF) * land_shape.most_sigmoidness.saturating_sub(land_shape.least_sigmoidness)) >> 16))
        };
        let lines = [line(0), line(1), line(2)];
        MeshTriangle { corner_vertices, blends: lines.map(|line| line.0), line_sigmoidness: lines.map(|line| line.1.clamp(SIGMOID_ONE, SIGMOID_MOST)), area_inverse }
    }

    /// The triangle the cell `moved_cartesian`, moved, is in, and how near each of
    /// its corners the cell is, of [`ONE`] and to [`ONE`] in all.
    fn locate_triangle_cartesian(&mut self, land_shape: &LandShape, seed: u64, moved_cartesian: (i64, i64)) -> (MeshTriangle, [u64; 3]) {
        let cell_square = (moved_cartesian.0.div_euclid(self.square_side), moved_cartesian.1.div_euclid(self.square_side));
        if self.kept_square != Some(cell_square) {
            (self.kept_square, self.last_triangle) = (Some(cell_square), None);
            self.kept_vertices = std::array::from_fn(|about_index| self.vertex(land_shape, seed, (cell_square.0 + about_index as i64 % 3 - 1, cell_square.1 + about_index as i64 / 3 - 1)));
        }
        // How near each corner, if the cell is within: each corner's share of the triangle's area.
        let corner_nearness = |tried_corners: [usize; 3], kept_inverse: Option<u64>| {
            let [first_corner_point, second_corner_point, third_corner_point] = tried_corners.map(|corner_index| self.kept_vertices[corner_index].vertex_cartesian);
            let (whole_area, first_part, second_part) = (area(first_corner_point, second_corner_point, third_corner_point), area(moved_cartesian, second_corner_point, third_corner_point), area(first_corner_point, moved_cartesian, third_corner_point));
            let third_part = whole_area - first_part - second_part;
            // On a line, a cell is in both its triangles: only the first looked at in a fixed order may claim it, never the one kept.
            let strictly_inside = kept_inverse.is_some();
            let cell_inside = whole_area != 0 && [first_part, second_part, third_part].iter().all(|&part| (part == 0 && !strictly_inside) || (part != 0 && (part > 0) == (whole_area > 0)));
            cell_inside.then(|| {
                let area_inverse = kept_inverse.unwrap_or_else(|| inverse_of_area(whole_area));
                let share_of_part = |part: i64| if area_inverse != 0 { (part.unsigned_abs() * area_inverse) >> 46 } else { ((part.unsigned_abs() as u128 * FRACTION_ONE as u128) / whole_area.unsigned_abs() as u128) as u64 };
                let (first_share, second_share) = (share_of_part(first_part).min(FRACTION_ONE), share_of_part(second_part).min(FRACTION_ONE));
                ([first_share, second_share, FRACTION_ONE.saturating_sub(first_share + second_share)], area_inverse)
            })
        };
        // The last cell's triangle, as likely as not.
        if let Some((kept_triangle, (kept_nearness, _))) = self.last_triangle.and_then(|previous_triangle| Some((previous_triangle, corner_nearness(previous_triangle.corner_vertices, Some(previous_triangle.area_inverse))?))) {
            return (kept_triangle, kept_nearness);
        }
        for quad_index in 0..4 {
            let quad_corner = quad_index % 2 + quad_index / 2 * 3;
            let (here_corner, east_corner, south_corner, far_corner) = (quad_corner, quad_corner + 1, quad_corner + 3, quad_corner + 4);
            // Cut along one diagonal or the other, by the quad's first vertex's lot.
            let quad_halves = if self.kept_vertices[here_corner].vertex_lot >> 63 == 0 { [[here_corner, east_corner, far_corner], [here_corner, far_corner, south_corner]] } else { [[here_corner, east_corner, south_corner], [east_corner, far_corner, south_corner]] };
            for half_corners in quad_halves {
                if let Some((found_nearness, found_inverse)) = corner_nearness(half_corners, None) {
                    let found_triangle = self.triangle_of_corners(land_shape, half_corners, found_inverse);
                    self.last_triangle = Some(found_triangle);
                    return (found_triangle, found_nearness);
                }
            }
        }
        // In none, by a rounding: at the square's own vertex.
        (MeshTriangle { corner_vertices: [4; 3], blends: [FRACTION_ONE; 3], line_sigmoidness: [SIGMOID_ONE; 3], area_inverse: 0 }, [FRACTION_ONE, 0, 0])
    }

    /// What the mesh says of the cell `moved_cartesian`, moved.
    fn blended_cartesian(&mut self, land_shape: &LandShape, seed: u64, moved_cartesian: (i64, i64)) -> BlendedCell {
        let (cell_triangle, corner_nearness) = self.locate_triangle_cartesian(land_shape, seed, moved_cartesian);
        let (mut heights, mut inland_sum, mut free_sum, mut counts_sum) = (0i64, 0u64, 0u64, 0u64);
        for corner_index in 0..3 {
            let (next_corner, other_corner) = ((corner_index + 1) % 3, (corner_index + 2) % 3);
            // Its two lines' blend and sigmoidness, each counting as the cell is nearer that line's other end.
            let towards_next = (corner_nearness[next_corner] << 16).checked_div(corner_nearness[next_corner] + corner_nearness[other_corner]).unwrap_or(FRACTION_ONE);
            let weighted_by_line = |line_values: &[u64; 3]| (line_values[corner_index] * towards_next + line_values[other_corner] * (FRACTION_ONE - towards_next)) >> 16;
            let (blend, cell_sigmoidness) = (weighted_by_line(&cell_triangle.blends).max(1), weighted_by_line(&cell_triangle.line_sigmoidness));
            // How near this corner beside the nearer of the others: a half where the two are as near.
            let corner_share = (corner_nearness[corner_index] << 16) / (corner_nearness[corner_index] + corner_nearness[next_corner].max(corner_nearness[other_corner])).max(1);
            // The change is all within the blend, about the half.
            let along_blend = (((corner_share as i64 - (FRACTION_ONE / 2) as i64 + (blend / 2) as i64) << 16) / blend as i64).clamp(0, FRACTION_ONE as i64) as u64;
            // Past the blend's ends, or on an even slope, there is no power to raise.
            let counts = if along_blend == 0 || along_blend == FRACTION_ONE || cell_sigmoidness == SIGMOID_ONE {
                along_blend
            } else {
                let (rising_part, falling_part) = (raised_to_power(along_blend, cell_sigmoidness), raised_to_power(FRACTION_ONE - along_blend, cell_sigmoidness));
                (rising_part << 16) / (rising_part + falling_part).max(1)
            };
            let vertex = &self.kept_vertices[cell_triangle.corner_vertices[corner_index]];
            (heights, inland_sum, free_sum, counts_sum) = (heights + counts as i64 * vertex.height, inland_sum + counts * vertex.inland_share, free_sum + counts * vertex.free_weight, counts_sum + counts);
        }
        let counts_total = counts_sum.max(1);
        BlendedCell { height: heights / counts_total as i64, inland_share: inland_sum / counts_total, free_weight: free_sum / counts_total, line: (corner_nearness.iter().min().copied().unwrap_or(0) * self.square_side as u64) >> 16 }
    }
}
