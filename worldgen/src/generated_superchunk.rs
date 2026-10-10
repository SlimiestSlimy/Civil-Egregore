//! A superchunk as it is generated: its cells, the terrain and what
//! grows on it put on the layers a world has ([`layer_types`]). The
//! image of them is storage's to make (`SuperchunkCells::image`).

use crate::{Generation, Terrain, WALLS};
use type_registry::{GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WET};
use bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::{ChunkMaps, LayerType, SuperchunkCells};
use coordinates::{cartesian_from_place, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};

/// The cells of `superchunk` in a world made from `seed` as
/// `generation` says: its terrain, the ocean where it is under the
/// ocean's level, and on the rest grass in patches -- dirt where there is none --
/// and trees in patches of their own, each of a stage drawn for its
/// cell -- the same whenever it is made.
pub fn generate_superchunk(generation: &Generation, seed: u64, superchunk: SuperchunkIndex) -> SuperchunkCells {
    let terrain = Terrain::generate_shaped(&generation.shape, seed, superchunk);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let growth = generation.growth(seed);
    // The planes generated, each a bitmap a chunk: grass, trees, their stage's four, and the cells under water.
    let mut planes = vec![GRASS, TREE];
    planes.extend(TREE_STAGE.layer_type().planes());
    let wet = planes.len();
    planes.push(WET);
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
    SuperchunkCells { water_depths: ChunkMaps::from_numbers(depth_at), heights: terrain.heights, layers }
}
