//! A superchunk as it is generated: its cells, the terrain and what
//! grows on it put on the layers a world has ([`layer_types`]). The
//! image of them is storage's to make (`SuperchunkCells::image`).

use crate::climate::ClimateNoises;
use crate::{Generation, Terrain, WALLS};
use type_registry::{CLIMATE, COLLISION, ELEVATION, GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::{ChunkMaps, LayerType, SuperchunkCells};
use coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};

/// The cells of `superchunk` in a world made from `seed` as
/// `generation` says: its terrain, the ocean where it is under the
/// ocean's level, and on the rest grass in patches -- dirt where there is none --
/// and trees in patches of their own, each of a stage drawn for its
/// cell; and over all of it the climate, a tile at a time
/// ([`ClimateNoises`]) -- the same whenever it is made.
pub fn generate_superchunk(generation: &Generation, seed: u64, superchunk: SuperchunkIndex) -> SuperchunkCells {
    let terrain = Terrain::generate_shaped(&generation.shape, seed, superchunk);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let mut growth = generation.growth(seed);
    // The planes generated, each a bitmap a chunk: grass, trees, their stage's four, the cells under water, and the collision plane.
    let mut planes = vec![GRASS, TREE];
    planes.extend(TREE_STAGE.layer_type().planes());
    let wet = planes.len();
    planes.push(WET);
    let collision = planes.len();
    planes.push(COLLISION);
    // The ocean wherever the ground is under its level, as deep as it is lower.
    let depth_at = |place: usize| generation.shape.ocean.saturating_sub(terrain.height(place));
    let mut cells = vec![[0u64; WORDS]; planes.len() * CHUNKS_IN_SUPERCHUNK];
    for place in 0..CHUNKS_IN_SUPERCHUNK * CELLS_IN_CHUNK {
        let (x, y) = cartesian_from_place(place);
        let (x, y) = (left + x, top + y);
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        let mut set = |plane: usize| cells[plane * CHUNKS_IN_SUPERCHUNK + chunk][cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
        // Nothing grows under water.
        if depth_at(place) > 0 {
            set(wet);
            continue;
        }
        let grown = growth.at(x, y);
        if grown.grass {
            set(0);
        }
        if let Some(lot) = grown.tree {
            set(1);
            // Where a tree stands is not stepped on.
            set(collision);
            // Its stage: a lot of the cell's own.
            let stage = lot % (OLDEST_TREE_STAGE as u64 + 1);
            (0..TREE_STAGE.layer_type().bits() as usize).filter(|bit| stage >> bit & 1 == 1).for_each(|bit| set(2 + bit));
        }
    }
    // A layer a chunk for each plane with a cell set on it, and for each way's walls.
    let mut layers: Vec<(usize, LayerType, CellWords)> = Vec::new();
    for (index, cells) in cells.iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
        layers.push((index % CHUNKS_IN_SUPERCHUNK, planes[index / CHUNKS_IN_SUPERCHUNK], *cells));
    }
    for (way, &(layer_type, _)) in WALLS.iter().enumerate() {
        for (place, cells) in terrain.walls[way].iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
            layers.push((place, layer_type, *cells));
        }
    }
    // The climate, a number a tile: each of its bits a plane, the tiles its first cells in Morton order.
    let (mut climate, tile_cells) = (ClimateNoises::new(seed), 1usize << CLIMATE.layer_type().tile_shift());
    // And how high each tile stands over the ocean, laid out the same: what the air's moves are held against.
    let climate_planes: Vec<LayerType> = CLIMATE.layer_type().planes().chain(ELEVATION.layer_type().planes()).collect();
    let climate_bits = CLIMATE.layer_type().bits() as usize;
    for chunk in 0..CHUNKS_IN_SUPERCHUNK {
        let mut bits = vec![[0u64; WORDS]; climate_planes.len()];
        for tile in 0..CELLS_IN_CHUNK / tile_cells {
            // The tile's middle cell: half a tile across and down from its first.
            let middle = chunk * CELLS_IN_CHUNK + tile * tile_cells + 3 * tile_cells / 4;
            let (x, y) = cartesian_from_place(middle);
            let above = terrain.height(middle).saturating_sub(generation.shape.ocean);
            let number = u64::from(climate.at(left + x, top + y, above, depth_at(middle) > 0).number()) | u64::from(u32::from(above).min(ELEVATION.most())) << climate_bits;
            (0..climate_planes.len()).filter(|bit| number >> bit & 1 == 1).for_each(|bit| bits[bit][tile / BITS_PER_WORD] |= 1 << (tile % BITS_PER_WORD));
        }
        layers.extend(bits.iter().zip(&climate_planes).filter(|(bits, _)| bits.iter().any(|&word| word != 0)).map(|(bits, &plane)| (chunk, plane, *bits)));
    }
    SuperchunkCells { water_depths: ChunkMaps::from_numbers(depth_at), heights: terrain.heights, layers }
}
