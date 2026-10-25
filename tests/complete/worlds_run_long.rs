//! Worlds run for long: stopped and saved often, they come to the same.

use protocol::server_parameters::{CleanServerParameters, ParameterValue, SEED};
use crate::worlds::{base, cells_of_grass, every_entity, nothing_more, one_flock_world, probe_and_base_game};
use server::world_hash;

/// A world run 30,000 ticks straight, and the same world saved and
/// loaded every 5,000: the same at the end, hashed whole.
#[test]
#[ignore]
fn a_world_stopped_often_comes_to_the_same() {
    const LAST_TICK: u64 = 30_000;
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let (mut straight_world, _) = one_flock_world("a_world_stopped_often_comes_to_the_same_straight", CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000, nothing_more);
    // The flock put in the first tick.
    for _ in 1..LAST_TICK {
        straight_world.tick();
    }
    let (mut stopped_world, _) = one_flock_world("a_world_stopped_often_comes_to_the_same_stopped", CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000, nothing_more);
    let folder = stopped_world.folder().to_path_buf();
    let mut tick = 1;
    for stop_tick in (5_000..=LAST_TICK).step_by(5_000) {
        while tick < stop_tick {
            stopped_world.tick();
            tick += 1;
        }
        server::save(&mut stopped_world).expect("saved");
        stopped_world = crate::worlds::opened(&folder, probe_and_base_game).expect("loaded");
    }
    // A cooled superchunk's last changes wait in the writeback ring: flushed, as a save's are, before the two are hashed.
    straight_world.write_back_and_flush_all();
    stopped_world.write_back_and_flush_all();
    let (straight_hash, stopped_hash) = (world_hash(&mut straight_world), world_hash(&mut stopped_world));
    assert_eq!(straight_hash.tick, LAST_TICK);
    assert_eq!(straight_hash, stopped_hash, "the same at tick {LAST_TICK}");
}

/// A flock on generated ground, walls and all, its halos following it,
/// neither dies out nor overruns its pasture in 300,000 ticks.
#[test]
#[ignore]
fn a_flock_on_generated_ground_lasts() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let (mut made_world, probe) = one_flock_world("a_flock_on_generated_ground_lasts", CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000, nothing_more);
    for _ in 0..300_000 {
        made_world.tick();
    }
    let (grass_cells, sheep_count) = (cells_of_grass(&mut made_world, &probe), every_entity(&mut made_world, &probe, |entity| entity.base_data.entity_type == base().sheep).into_iter().filter(|&is_sheep| is_sheep).count());
    assert!((500..200_000).contains(&sheep_count), "{sheep_count} sheep");
    assert!(grass_cells > 4 * 1024 * 1024 / 20, "{grass_cells} cells of grass");
}
