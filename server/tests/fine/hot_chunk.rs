//! A hot chunk's planes: what is written reads back, tile by tile,
//! and the tiles not 0 are counted.

use server::hot_chunk::HotChunk;
use std::collections::HashMap;
use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// Planes of every width and tile size drawn, written at cells drawn: every
/// tile reads what was last written to any of its cells, and each plane
/// counts its tiles not 0 as a map of them does; emptied planes, let go,
/// read 0.
#[test]
fn tiles_read_back_and_are_counted() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let chunk_index = ChunkIndex(random_numbers.draw_below(1 << 20));
    let mut chunk = HotChunk::new(chunk_index, seed);
    // Each plane: its ID, width, tile size, and its tiles not 0 by their first cell.
    let mut planes: Vec<(u64, u32, u32, HashMap<usize, u32>)> = Vec::new();
    for _ in 0..6 {
        let width_bits = 1 << random_numbers.draw_below(6);
        let tile_size = 1 << random_numbers.draw_below(9);
        planes.push((random_numbers.next_draw(), width_bits, tile_size, HashMap::new()));
    }
    for step in 0..20_000 {
        let (plane, width_bits, tile_size, model) = &mut planes[random_numbers.draw_below(6) as usize];
        let place = random_numbers.draw_below(CELLS_IN_CHUNK as u64) as usize;
        let cell = CellIndex::from_chunk_place(chunk_index, place);
        let tile = place / (*tile_size * *tile_size) as usize;
        let value = if random_numbers.draw_below(3) == 0 { 0 } else { (random_numbers.next_draw() & (u64::MAX >> (64 - *width_bits))) as u32 };
        assert_eq!(chunk.read_tile(*plane, cell), model.get(&tile).copied().unwrap_or(0), "step {step}");
        chunk.write_tile(*plane, *width_bits, *tile_size, cell, value);
        if value == 0 {
            model.remove(&tile);
        } else {
            model.insert(tile, value);
        }
        assert_eq!(chunk.read_tile(*plane, cell), value, "step {step}");
        assert_eq!(chunk.non_zero_tiles(*plane), model.len() as u32, "step {step}");
    }
    for (plane, _, tile_size, model) in &mut planes {
        for (&tile, &value) in model.iter() {
            let last_cell = CellIndex::from_chunk_place(chunk_index, (tile + 1) * (*tile_size * *tile_size) as usize - 1);
            assert_eq!(chunk.read_tile(*plane, last_cell), value);
        }
        let tiles: Vec<usize> = model.keys().copied().collect();
        for tile in tiles {
            chunk.write_tile(*plane, 1, *tile_size, CellIndex::from_chunk_place(chunk_index, tile * (*tile_size * *tile_size) as usize), 0);
        }
        model.clear();
    }
    chunk.drop_empty_planes();
    for (plane, ..) in &planes {
        assert_eq!(chunk.non_zero_tiles(*plane), 0);
        assert_eq!(chunk.read_tile(*plane, chunk_index.top_left()), 0);
    }
}
