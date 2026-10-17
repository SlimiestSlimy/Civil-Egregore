//! The land asked of a cell at a time: the mesh behind it, what it last
//! found kept.

use crate::{FRACTION_ONE, LandShape, noise_at_cell_cartesian};
use super::{FINER_MOST, FINEST_SPACING_POWER, Mesh, WARP_INDEX};
use base_types::height_map::Height;

/// The cells from one point of the lattice to the next, as a power of
/// two: the meshes are worked out at its points alone, a sixteenth of
/// the cells, and no line of a mesh is shorter than four of its steps.
const LATTICE_STEP_POWER: u32 = 2;
/// The cells from one point of the lattice to the next.
const LATTICE_STEP: u32 = 1 << LATTICE_STEP_POWER;

/// The land `cells_across` cells east and `cells_down` south of the first of four
/// points of the lattice -- top left, top right, bottom left, bottom
/// right, each in 16-bit fractions of a height -- in heights: each
/// point's by how near it the cell is, made whole once.
fn between_the_lattice(corner_points: [i64; 4], cells_across: u32, cells_down: u32) -> u64 {
    let (cells_across, cells_down, step) = (cells_across as i64, cells_down as i64, LATTICE_STEP as i64);
    let upper_row = corner_points[0] * (step - cells_across) + corner_points[1] * cells_across;
    let lower_row = corner_points[2] * (step - cells_across) + corner_points[3] * cells_across;
    let blended_land = (upper_row * (step - cells_down) + lower_row * cells_down) >> (2 * LATTICE_STEP_POWER);
    ((blended_land + (FRACTION_ONE / 2) as i64) >> 16) as u64
}

/// The land of a world, asked for cell after cell: the vertices about
/// the last cell and its triangle are kept, for the next is nearly
/// always in the same -- so they are drawn once for a square of a
/// grid, not once for a cell.
pub struct WorldLand {
    /// How the heights are shaped.
    land_shape: LandShape,
    /// The world's seed.
    seed: u64,
    /// The cells along a square of the broadest grid, as a power of two.
    broad_spacing_power: u32,
    /// Cells a line is bent by at most.
    bend_cells: i64,
    /// The land's and the ocean's mesh.
    broad_mesh: Mesh,
    /// The finer meshes, the broadest first.
    finer_meshes: [Mesh; FINER_MOST],
}

impl WorldLand {
    /// The land of the world of `seed`, shaped as `land_shape` says.
    pub fn new(land_shape: &LandShape, seed: u64) -> Self {
        let broad_spacing_power = land_shape.vertex_spacing_power.clamp(FINEST_SPACING_POWER, 24);
        Self { land_shape: *land_shape, seed, broad_spacing_power, bend_cells: (((1u64 << broad_spacing_power) * land_shape.line_bending) >> 16) as i64, broad_mesh: Mesh::new(land_shape, 0), finer_meshes: std::array::from_fn(|finer_index| Mesh::new(land_shape, finer_index as u32 + 1)) }
    }

    /// The cell `(x, y)` moved by broad noise: what bends the lines.
    fn moved_cartesian(&self, x: u32, y: u32) -> (i64, i64) {
        let bend_by = |noise_index: u32| ((noise_at_cell_cartesian(self.seed, noise_index, self.broad_spacing_power - 2, x, y) as i64 - (FRACTION_ONE / 2) as i64) * 2 * self.bend_cells) >> 16;
        (x as i64 + bend_by(WARP_INDEX), y as i64 + bend_by(WARP_INDEX + 1))
    }

    /// The land at the lattice's point `(x, y)`, in 16-bit fractions
    /// of a height: the broad mesh's height there, and what the finer
    /// meshes raise or sink it by, as much of it as the land there
    /// stands high -- never under the lowest ground.
    fn at_the_lattice(&mut self, x: u32, y: u32) -> i64 {
        let moved_point = self.moved_cartesian(x, y);
        let broad_blended = self.broad_mesh.blended_cartesian(&self.land_shape, self.seed, moved_point);
        let lowest_ground = (self.land_shape.ocean_floor as i64) << 16;
        if broad_blended.inland_share == 0 {
            return (broad_blended.height << 16).max(lowest_ground);
        }
        // No finer than the finest: a mesh that would be is left out.
        let finer_count = (self.land_shape.finer_depth as usize).min(FINER_MOST).min((self.broad_spacing_power - FINEST_SPACING_POWER) as usize);
        // The broad mesh has the whole weight; each mesh hands a share of what reached it on to the next, and
        // moves the land by its own heights times what reached it.
        let (mut finer_moved, mut free_weight) = (0i64, (broad_blended.inland_share * broad_blended.free_weight) >> 16);
        for mesh in self.finer_meshes.iter_mut().take(finer_count) {
            let mesh_blended = mesh.blended_cartesian(&self.land_shape, self.seed, moved_point);
            (finer_moved, free_weight) = (finer_moved + mesh_blended.height * free_weight as i64, (free_weight * mesh_blended.free_weight) >> 16);
        }
        ((broad_blended.height << 16) + finer_moved).max(lowest_ground)
    }

    /// The land at the cell `(x, y)`, in heights: the meshes' at the
    /// lattice's four points about it, blended by how near each the
    /// cell is (`docs/worldgen.md`, "On a lattice").
    pub fn land_at_cartesian(&mut self, x: u32, y: u32) -> u64 {
        let (lattice_left, lattice_top) = (x & !(LATTICE_STEP - 1), y & !(LATTICE_STEP - 1));
        let (cells_across, cells_down) = (x - lattice_left, y - lattice_top);
        // A point no share of the cell is taken from is not worked out.
        let mut lattice_point = |east_point: bool, south_point: bool| if (east_point && cells_across == 0) || (south_point && cells_down == 0) { 0 } else { self.at_the_lattice(lattice_left.wrapping_add(east_point as u32 * LATTICE_STEP), lattice_top.wrapping_add(south_point as u32 * LATTICE_STEP)) };
        between_the_lattice([lattice_point(false, false), lattice_point(true, false), lattice_point(false, true), lattice_point(true, true)], cells_across, cells_down)
    }

    /// The heights of the square of `side_cells` cells whose top left cell
    /// is `(square_left, square_top)`, row by row: each what [`WorldLand::height`] says,
    /// the lattice's points worked out once each.
    pub fn heights_of_a_square(&mut self, square_left: u32, square_top: u32, side_cells: u32) -> Vec<Height> {
        let (first_across, first_down) = (square_left & !(LATTICE_STEP - 1), square_top & !(LATTICE_STEP - 1));
        let points_a_side = ((square_left - first_across + side_cells - 1) / LATTICE_STEP + 2) as usize;
        let mut lattice_lands = Vec::with_capacity(points_a_side * points_a_side);
        for point_down in 0..points_a_side as u32 {
            for point_across in 0..points_a_side as u32 {
                lattice_lands.push(self.at_the_lattice(first_across.wrapping_add(point_across * LATTICE_STEP), first_down.wrapping_add(point_down * LATTICE_STEP)));
            }
        }
        let mut heights = Vec::with_capacity((side_cells * side_cells) as usize);
        for y in 0..side_cells {
            let (lattice_row, cells_down) = (((square_top - first_down + y) / LATTICE_STEP) as usize, (square_top - first_down + y) % LATTICE_STEP);
            for x in 0..side_cells {
                let (lattice_column, cells_across) = (((square_left - first_across + x) / LATTICE_STEP) as usize, (square_left - first_across + x) % LATTICE_STEP);
                let point_index = lattice_row * points_a_side + lattice_column;
                let blended_land = between_the_lattice([lattice_lands[point_index], lattice_lands[point_index + 1], lattice_lands[point_index + points_a_side], lattice_lands[point_index + points_a_side + 1]], cells_across, cells_down);
                heights.push(blended_land.min(Height::MAX as u64) as Height);
            }
        }
        heights
    }

    /// The height of the cell `(x, y)`: its land, and no more than the
    /// highest there is, whatever the shape would come to.
    pub fn height(&mut self, x: u32, y: u32) -> Height {
        self.land_at_cartesian(x, y).min(Height::MAX as u64) as Height
    }

    /// How high the land at the cell `(x, y)` stands of what it may, of
    /// [`ONE`], and about
    /// how many cells it is from the nearest line of the broad mesh.
    pub fn inland_and_line_distance(&mut self, x: u32, y: u32) -> (u64, u64) {
        let moved_point = self.moved_cartesian(x, y);
        let broad_blended = self.broad_mesh.blended_cartesian(&self.land_shape, self.seed, moved_point);
        (broad_blended.inland_share, broad_blended.line)
    }
}

/// The land at the cell `(x, y)`: [`WorldLand::land_at_cartesian`], for one cell alone.
pub fn land_of_one_cell_cartesian(land_shape: &LandShape, seed: u64, x: u32, y: u32) -> u64 {
    WorldLand::new(land_shape, seed).land_at_cartesian(x, y)
}
