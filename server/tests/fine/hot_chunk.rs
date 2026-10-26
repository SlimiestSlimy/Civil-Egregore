//! A hot chunk's planes: what is written reads back, tile by tile,
//! and the tiles not 0 are counted.

use server::hot_chunk::HotChunk;
use server::plane_shapes::PlaneShape;
use std::collections::HashMap;
use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// Planes of every width and tile scale drawn, written at cells drawn: every
/// tile reads what was last written to any of its cells, and each plane
/// counts its tiles not 0 as a map of them does; emptied planes, let go,
/// read 0.
#[test]
fn tiles_read_back_and_are_counted() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let chunk_index = ChunkIndex(random_numbers.draw_below(1 << 20));
    let mut chunk = HotChunk::new(chunk_index, seed);
    // Each plane: its type ID, its shape, and its tiles not 0 by their number.
    let mut planes: Vec<(u64, PlaneShape, HashMap<usize, u32>)> = Vec::new();
    for _ in 0..6 {
        let shape = PlaneShape { width_bits: 1 << random_numbers.draw_below(6), tile_scale: random_numbers.draw_below(9) as u32 };
        planes.push((random_numbers.next_draw(), shape, HashMap::new()));
    }
    for step in 0..20_000 {
        let (plane_type_id, shape, model) = &mut planes[random_numbers.draw_below(6) as usize];
        let place = random_numbers.draw_below(CELLS_IN_CHUNK as u64) as usize;
        let cell = CellIndex::from_chunk_place(chunk_index, place);
        let tile = place / shape.tile_cells();
        let value = if random_numbers.draw_below(3) == 0 { 0 } else { (random_numbers.next_draw() & (u64::MAX >> (64 - shape.width_bits))) as u32 };
        assert_eq!(chunk.read_tile(*plane_type_id, cell), model.get(&tile).copied().unwrap_or(0), "step {step}");
        chunk.write_tile(*plane_type_id, *shape, cell, value);
        if value == 0 {
            model.remove(&tile);
        } else {
            model.insert(tile, value);
        }
        assert_eq!(chunk.read_tile(*plane_type_id, cell), value, "step {step}");
        assert_eq!(chunk.non_zero_tiles(*plane_type_id), model.len() as u32, "step {step}");
    }
    for (plane_type_id, shape, model) in &mut planes {
        for (&tile, &value) in model.iter() {
            let last_cell = CellIndex::from_chunk_place(chunk_index, (tile + 1) * shape.tile_cells() - 1);
            assert_eq!(chunk.read_tile(*plane_type_id, last_cell), value);
        }
        let tiles: Vec<usize> = model.keys().copied().collect();
        for tile in tiles {
            chunk.write_tile(*plane_type_id, *shape, CellIndex::from_chunk_place(chunk_index, tile * shape.tile_cells()), 0);
        }
        model.clear();
    }
    chunk.drop_empty_planes();
    for (plane_type_id, ..) in &planes {
        assert_eq!(chunk.non_zero_tiles(*plane_type_id), 0);
        assert_eq!(chunk.read_tile(*plane_type_id, chunk_index.top_left()), 0);
    }
}
