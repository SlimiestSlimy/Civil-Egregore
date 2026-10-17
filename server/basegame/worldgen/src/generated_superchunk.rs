//! A superchunk as it is generated: its cells, the terrain and what
//! grows on it put on the layers a world has ([`layer_types`]). The
//! image of them is storage's to make (`SuperchunkCells::image`).

use crate::climate::ClimateNoises;
use crate::{WorldGeneration, Terrain, WALLS};
use base_types::{CLIMATE, COLLISION, ELEVATION, GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::{ChunkMaps, LayerType, SuperchunkCells};
use utilities::coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};

/// The cells of `superchunk` in a world made from `seed` as
/// `world_generation` says: its terrain, the ocean where it is under the
/// ocean's level, and on the rest grass in patches -- dirt where there is none --
/// and trees in patches of their own, each of a stage drawn for its
/// cell; and over all of it the climate, a tile at a time
/// ([`ClimateNoises`]) -- the same whenever it is made.
pub fn generate_superchunk(world_generation: &WorldGeneration, seed: u64, superchunk: SuperchunkIndex) -> SuperchunkCells {
    let terrain = Terrain::generate_shaped(&world_generation.land_shape, seed, superchunk);
    let CellCartesian { x: superchunk_left, y: superchunk_top } = superchunk.top_left().cartesian();
    let mut cell_growth = world_generation.world_growth(seed);
    // The planes generated, each a bitmap a chunk: grass, trees, their stage's four, the cells under water, and the collision plane.
    let mut generated_planes = vec![GRASS, TREE];
    generated_planes.extend(TREE_STAGE.layer_type().each_bitplane());
    let wet_index = generated_planes.len();
    generated_planes.push(WET);
    let collision_index = generated_planes.len();
    generated_planes.push(COLLISION);
    // The ocean wherever the ground is under its level, as deep as it is lower.
    let depth_at = |place: usize| world_generation.land_shape.ocean.saturating_sub(terrain.height(place));
    let mut cells = vec![[0u64; WORDS]; generated_planes.len() * CHUNKS_IN_SUPERCHUNK];
    for place in 0..CHUNKS_IN_SUPERCHUNK * CELLS_IN_CHUNK {
        let (x, y) = cartesian_from_place(place);
        let (x, y) = (superchunk_left + x, superchunk_top + y);
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        let mut set_bit = |plane_index: usize| cells[plane_index * CHUNKS_IN_SUPERCHUNK + chunk][cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
        // Nothing grows under water.
        if depth_at(place) > 0 {
            set_bit(wet_index);
            continue;
        }
        let grown_cell = cell_growth.grown_at(x, y);
        if grown_cell.has_grass {
            set_bit(0);
        }
        if let Some(tree_lot) = grown_cell.tree {
            set_bit(1);
            // Where a tree stands is not stepped on.
            set_bit(collision_index);
            // Its stage: a lot of the cell's own.
            let stage = tree_lot % (OLDEST_TREE_STAGE as u64 + 1);
            (0..TREE_STAGE.layer_type().width_bits() as usize).filter(|stage_bit| stage >> stage_bit & 1 == 1).for_each(|stage_bit| set_bit(2 + stage_bit));
        }
    }
    // A layer a chunk for each plane with a cell set on it, and for each way's walls.
    let mut layers: Vec<(usize, LayerType, CellWords)> = Vec::new();
    for (bitmap_index, cells) in cells.iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
        layers.push((bitmap_index % CHUNKS_IN_SUPERCHUNK, generated_planes[bitmap_index / CHUNKS_IN_SUPERCHUNK], *cells));
    }
    for (way, &(layer_type, _)) in WALLS.iter().enumerate() {
        for (place, cells) in terrain.walls[way].iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
            layers.push((place, layer_type, *cells));
        }
    }
    // The climate, a number a tile: each of its bits a plane, the tiles its first cells in Morton order.
    let (mut climate, tile_cells) = (ClimateNoises::new(seed), 1usize << CLIMATE.layer_type().tile_shift());
    // And how high each tile stands over the ocean, laid out the same: what the air's moves are held against.
    let climate_planes: Vec<LayerType> = CLIMATE.layer_type().each_bitplane().chain(ELEVATION.layer_type().each_bitplane()).collect();
    let climate_bits = CLIMATE.layer_type().width_bits() as usize;
    for chunk in 0..CHUNKS_IN_SUPERCHUNK {
        let mut climate_bitmaps = vec![[0u64; WORDS]; climate_planes.len()];
        for tile in 0..CELLS_IN_CHUNK / tile_cells {
            // The tile's middle cell: half a tile across and down from its first.
            let middle_place = chunk * CELLS_IN_CHUNK + tile * tile_cells + 3 * tile_cells / 4;
            let (x, y) = cartesian_from_place(middle_place);
            let height_above_ocean = terrain.height(middle_place).saturating_sub(world_generation.land_shape.ocean);
            let tile_number = u64::from(climate.climate_at(superchunk_left + x, superchunk_top + y, height_above_ocean, depth_at(middle_place) > 0).plane_number()) | u64::from(u32::from(height_above_ocean).min(ELEVATION.most_value())) << climate_bits;
            (0..climate_planes.len()).filter(|plane_bit| tile_number >> plane_bit & 1 == 1).for_each(|plane_bit| climate_bitmaps[plane_bit][tile / BITS_PER_WORD] |= 1 << (tile % BITS_PER_WORD));
        }
        layers.extend(climate_bitmaps.iter().zip(&climate_planes).filter(|(plane_bitmap, _)| plane_bitmap.iter().any(|&word| word != 0)).map(|(plane_bitmap, &plane_type)| (chunk, plane_type, *plane_bitmap)));
    }
    SuperchunkCells { water_depths: ChunkMaps::from_numbers(depth_at), height_words: terrain.heights.words().into(), layers }
}
