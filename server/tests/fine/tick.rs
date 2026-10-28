//! The tick: grass sampled, dying and spreading by its sets, on two hot
//! chunks; the same seed, the same grass.

use protocol::type_kinds::Kind;
use server::hot_chunk::HotChunk;
use server::sca_proc::ScaProc;
use server::server_handle::ServerHandle;
use server::server_manager::{LoadedScaProc, ServerManager};
use utilities::coordinates::{place_from_cartesian, CellIndex, ChunkIndex, CHUNK_SIDE_CELLS};
use utilities::rng::RandomSource;

/// The mods the tests register: grass, a plane, and the sca proc on it.
const MANIFESTS: [&str; 2] = [
    ":planes:\n- side : common\n- version : 0\n\ngrass :\n- kind : plane\n- width : 1\n- tile side : 1\n",
    ":procs:\n- side : server\n- version : 0\n- depends on :\n    -- planes : 0\n\ngrass_dies_or_spreads :\n- kind : sca proc\n- planes :\n    -- planes>grass\n- seconds per sample : 0.01\n- set up : GRASS_DIES_OR_SPREADS_SET_UP\n",
];

/// Grass, sampled: dies, one time in ten; spreads to a neighbour of the
/// four, drawn, one time in eight -- in its chunk, sets across chunks to
/// come.
struct GrassDiesOrSpreads {
    /// Grass's type ID.
    grass: u64,
}

impl ScaProc for GrassDiesOrSpreads {
    fn call(&self, server: &mut ServerHandle<'_>, _sampled_plane_type_id: u64, tile_corner: CellIndex) {
        let roll = server.random_stream().draw_below(40);
        if roll < 4 {
            server.set().compare(self.grass, tile_corner, 1).write(self.grass, tile_corner, 0).end();
        } else if roll < 9 {
            let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][server.random_stream().draw_below(4) as usize];
            let Some(spread_to) = tile_corner.at_cartesian_offset(dx, dy).filter(|cell| cell.chunk() == tile_corner.chunk()) else { return };
            server.set().compare(self.grass, tile_corner, 1).compare(self.grass, spread_to, 0).write(self.grass, spread_to, 1).end();
        }
    }
}

/// A world of `MANIFESTS`, two chunks side by side hot in it -- drawn
/// from `seed`, a patch of grass in the middle of each -- ticked
/// `ticks` times: each chunk's grass after, and the grass at first.
fn grass_after_ticks(seed: u64, ticks: u32) -> (Vec<Vec<CellIndex>>, Vec<Vec<CellIndex>>) {
    let type_registry = crate::test_registry::registry_of_mods("tick", &MANIFESTS);
    let grass = type_registry.type_id_of("planes>grass").expect("grass registered");
    let proc_type_id = type_registry.type_id_of("procs>grass_dies_or_spreads").expect("the proc registered");
    let Kind::ScaProc { mean_simulation_seconds_per_sample, .. } = type_registry.type_of(proc_type_id).kind() else { panic!("a sca proc") };
    let sca_proc = LoadedScaProc { type_id: proc_type_id, sampled_plane_type_ids: vec![grass], chance_per_tick: mean_simulation_seconds_per_sample.chance_per_tick(), code: Box::new(GrassDiesOrSpreads { grass }) };
    let mut random_numbers = RandomSource::new(seed);
    let first_chunk = ChunkIndex(random_numbers.draw_below(1 << 20) & !1);
    let mut chunks = [first_chunk, ChunkIndex(first_chunk.0 + 1)].map(|chunk_index| HotChunk::new(chunk_index, seed));
    let middle = CHUNK_SIDE_CELLS / 2;
    for chunk in &mut chunks {
        for (x, y) in (0..8).flat_map(|x| (0..8).map(move |y| (x, y))) {
            if random_numbers.percent_chance(50) {
                let cell = CellIndex::from_chunk_place(chunk.chunk_index(), place_from_cartesian(middle + x, middle + y));
                chunk.write_tile(grass, cell, 1, &type_registry);
            }
        }
    }
    let grass_of = |chunk: &HotChunk| chunk.corners_of_non_zero_tiles(grass).collect::<Vec<_>>();
    let grass_at_first = chunks.iter().map(grass_of).collect();
    let mut server_manager = ServerManager::new(type_registry, vec![sca_proc]);
    chunks.into_iter().for_each(|chunk| server_manager.insert_hot_chunk(chunk));
    for _ in 0..ticks {
        server_manager.tick();
    }
    let grass_after = [first_chunk, ChunkIndex(first_chunk.0 + 1)].map(|chunk_index| grass_of(server_manager.hot_chunk(chunk_index).expect("still hot"))).to_vec();
    (grass_after, grass_at_first)
}

/// Over 16 ticks, on each chunk, grass has died where it was and spread
/// where it was not; and the same seed gives the same grass.
#[test]
fn grass_dies_spreads_and_is_the_same_of_the_same_seed() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (grass_after, grass_at_first) = grass_after_ticks(seed, 16);
    for (after, at_first) in grass_after.iter().zip(&grass_at_first) {
        assert!(at_first.iter().any(|cell| !after.contains(cell)), "some grass died");
        assert!(after.iter().any(|cell| !at_first.contains(cell)), "some grass spread");
    }
    assert_eq!(grass_after_ticks(seed, 16).0, grass_after, "the same seed, the same grass");
}
