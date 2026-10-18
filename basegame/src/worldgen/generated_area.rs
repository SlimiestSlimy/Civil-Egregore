//! An area as it is generated: its cells, the terrain and what grows on
//! it put on the planes a world has, a cell at a time, through the
//! simulation's API (`GeneratedArea`): what they are kept in is not
//! the generation's to know.

use crate::worldgen::{WorldGeneration, Terrain};
use crate::base_types::{BaseTypes, OLDEST_TREE_STAGE};
use server::simulation::simulation_api::{CellCartesian, GeneratedArea};

/// Writes the cells of `generated_area`'s area in a world made from
/// `seed`, on the planes of `base_types`, as `world_generation` says: its terrain -- every cell's height
/// and walls -- the ocean where it is under the ocean's level, as deep
/// as it is lower, and on the rest grass in patches -- dirt where there
/// is none -- and trees in patches of their own, each of a stage drawn
/// for its cell -- the same whenever it is made.
pub fn generate_area(base_types: &BaseTypes, world_generation: &WorldGeneration, seed: u64, generated_area: &mut GeneratedArea) {
    let area = generated_area.area();
    let terrain = Terrain::generate_shaped(&world_generation.land_shape, seed, area);
    let ocean = world_generation.land_shape.ocean;
    let mut cell_growth = world_generation.world_growth(seed);
    for (y, x) in (0..area.side_cells).flat_map(|y| (0..area.side_cells).map(move |x| (y, x))) {
        let cell = CellCartesian { x: area.top_left.x + x, y: area.top_left.y + y };
        let height = terrain.height(x, y);
        generated_area.set_value(base_types.height, cell, height.into());
        for (way, wall_layer) in base_types.walls().into_iter().enumerate() {
            if terrain.keeps_wall(way, x, y) {
                generated_area.set(wall_layer, cell);
            }
        }
        // Nothing grows under water.
        let water_depth = ocean.saturating_sub(height);
        if water_depth > 0 {
            generated_area.set_value(base_types.water_depth, cell, water_depth.into());
            generated_area.set(base_types.wet, cell);
            continue;
        }
        let grown_cell = cell_growth.grown_at(cell.x, cell.y);
        if grown_cell.has_grass {
            generated_area.set(base_types.grass, cell);
        }
        if let Some(tree_lot) = grown_cell.tree {
            generated_area.set(base_types.tree, cell);
            // Where a tree stands is not stepped on.
            generated_area.set(base_types.collision, cell);
            // Its stage: a lot of the cell's own.
            generated_area.set_value(base_types.tree_stage, cell, (tree_lot % (OLDEST_TREE_STAGE as u64 + 1)) as u32);
        }
    }
}
