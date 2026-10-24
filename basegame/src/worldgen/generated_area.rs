//! A superchunk as it is generated: its cells, the terrain and what
//! grows on it put on the planes a world has, a cell at a time, through
//! the arena's own writes (`GeneratingSuperchunk::cells_mut`): what they
//! are kept in is not the generation's to know.

use crate::worldgen::{WorldGeneration, Terrain};
use crate::base_types::{BaseTypes, OLDEST_TREE_STAGE};
use server::simulation::simulation_api::{CellArea, CellCartesian, CellIndex, GeneratingSuperchunk, Write, WriteOp, WritesApplied, SUPERCHUNK_SIDE_CELLS};

/// Writes the cells of `generating` in a world made from `seed`, on the
/// planes of `base_types`, as `world_generation` says: its terrain -- every
/// cell's height and walls -- the ocean where it is under the ocean's
/// level, as deep as it is lower, and on the rest grass in patches --
/// dirt where there is none -- and trees in patches of their own, each
/// of a stage drawn for its cell, held on the collision plane -- the
/// same whenever it is made.
pub fn generate_area(base_types: &BaseTypes, world_generation: &WorldGeneration, seed: u64, generating: &mut GeneratingSuperchunk) {
    let area = CellArea { top_left: generating.superchunk_index().top_left().cartesian(), side_cells: SUPERCHUNK_SIDE_CELLS };
    let terrain = Terrain::generate_shaped(&world_generation.land_shape, seed, area);
    let ocean = world_generation.land_shape.ocean;
    let mut cell_growth = world_generation.world_growth(seed);
    let mut writes_applied = WritesApplied::default();
    let cells = generating.cells_mut();
    for (y, x) in (0..area.side_cells).flat_map(|y| (0..area.side_cells).map(move |x| (y, x))) {
        let cell_cartesian = CellCartesian { x: area.top_left.x + x, y: area.top_left.y + y };
        let cell = CellIndex::from(cell_cartesian);
        let height = terrain.height(x, y);
        cells.apply(base_types.height.layer_type(), Write::value(base_types.height, cell, height.into()), &mut writes_applied);
        for (way, wall_layer) in base_types.walls().into_iter().enumerate() {
            if terrain.keeps_wall(way, x, y) {
                cells.apply(wall_layer, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
            }
        }
        // Nothing grows under water.
        let water_depth = ocean.saturating_sub(height);
        if water_depth > 0 {
            cells.apply(base_types.water_depth.layer_type(), Write::value(base_types.water_depth, cell, water_depth.into()), &mut writes_applied);
            cells.apply(base_types.wet, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
            continue;
        }
        let grown_cell = cell_growth.grown_at(cell_cartesian.x, cell_cartesian.y);
        if grown_cell.has_grass {
            cells.apply(base_types.grass, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
        }
        if let Some(tree_lot) = grown_cell.tree {
            cells.apply(base_types.tree, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
            // Where a tree stands is not stepped on.
            cells.apply(base_types.collision, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
            // Its stage: a lot of the cell's own.
            cells.apply(base_types.tree_stage.layer_type(), Write::value(base_types.tree_stage, cell, (tree_lot % (OLDEST_TREE_STAGE as u64 + 1)) as u32), &mut writes_applied);
        }
    }
    debug_assert_eq!(writes_applied.missed, 0, "every cell of the superchunk generated is hot");
}
