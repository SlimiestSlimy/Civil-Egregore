//! Sampling a hot chunk's plane: tiles not 0 alone, each its corner cell,
//! in Morton order, at the chance given.

use server::hot_chunk::HotChunk;
use server::plane_sampling::sample_plane;
use utilities::chance::Chance;
use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// A plane of tiles drawn not 0, sampled: always, each tile not 0 by its
/// corner cell, in Morton order; never, none; at a quarter, about a
/// quarter of them; a plane the chunk has not, none.
#[test]
fn tiles_not_zero_sampled_at_the_chance() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let chunk_index = ChunkIndex(random_numbers.draw_below(1 << 20));
    let mut chunk = HotChunk::new(chunk_index, seed);
    let side_cells = 1 << random_numbers.draw_below(5);
    let (type_registry, plane_type_ids) = crate::test_registry::registry_of_planes("plane_sampling", &[(8, side_cells)]);
    let plane_type_id = plane_type_ids[0];
    let tile_cells = (side_cells * side_cells) as usize;
    // Tiles drawn, one written at a cell drawn inside it.
    let mut non_zero_tiles: Vec<CellIndex> = (0..4000).map(|_| random_numbers.draw_below((CELLS_IN_CHUNK / tile_cells) as u64) as usize).map(|tile| CellIndex::from_chunk_place(chunk_index, tile * tile_cells)).collect();
    non_zero_tiles.sort();
    non_zero_tiles.dedup();
    for &tile in &non_zero_tiles {
        let cell_in_it = CellIndex(tile.0 + random_numbers.draw_below(tile_cells as u64));
        chunk.write_tile(plane_type_id, cell_in_it, 1 + random_numbers.draw_below(255) as u32, &type_registry);
    }
    let hits_at = |chance: Chance, random_stream: &mut RandomSource| {
        let mut hits = Vec::new();
        sample_plane(&chunk, plane_type_id, chance, random_stream, &mut hits);
        hits
    };
    assert_eq!(hits_at(Chance::ALWAYS_HAPPENS, &mut random_numbers), non_zero_tiles);
    assert_eq!(hits_at(Chance::NEVER_HAPPENS, &mut random_numbers), []);
    let quarter_hits = hits_at(Chance::one_in(4), &mut random_numbers);
    assert!(quarter_hits.iter().all(|hit| non_zero_tiles.binary_search(hit).is_ok()) && quarter_hits.is_sorted());
    // A quarter of ~4000, give or take far more than chance would stray.
    let expected = non_zero_tiles.len() / 4;
    assert!(quarter_hits.len().abs_diff(expected) < expected / 5, "{} of {}", quarter_hits.len(), non_zero_tiles.len());
    let mut no_hits = Vec::new();
    sample_plane(&chunk, plane_type_id ^ 1, Chance::ALWAYS_HAPPENS, &mut random_numbers, &mut no_hits);
    assert_eq!(no_hits, []);
}
