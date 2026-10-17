//! Worlds run for long: stopped and saved often, they come to the same.

use type_registry::GRASS;
use crate::worlds::{world_contents, folder};

/// A world run 30,000 ticks straight, and the same world saved and
/// loaded every 5,000: the same at the end.
#[test]
#[ignore]
fn a_world_stopped_often_comes_to_the_same() {
    const LAST_TICK: u64 = 30_000;
    let seed = server::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT);
    let mut straight_world = server::start_world(server::WorldStart { seed, sheep_per_flock: 4_000, ..server::WorldStart::default() });
    while straight_world.entity_store().current_tick() < LAST_TICK {
        straight_world.tick();
    }
    let folder = folder("complete");
    let mut stopped_world = server::start_world(server::WorldStart { seed, sheep_per_flock: 4_000, ..server::WorldStart::default() });
    for stop_tick in (5_000..=LAST_TICK).step_by(5_000) {
        while stopped_world.entity_store().current_tick() < stop_tick {
            stopped_world.tick();
        }
        server::save(&folder, &mut stopped_world).expect("saved");
        stopped_world = server::load_world(&folder).expect("loaded");
    }
    // A cooled superchunk's last changes wait in the writeback ring: flushed, as a save's are, before the images are set side by side.
    straight_world.write_back_and_flush_all();
    assert!(world_contents(&straight_world) == world_contents(&stopped_world), "the same at tick {LAST_TICK}");
}

/// A flock on generated ground, walls and all, its halos following it,
/// neither dies out nor overruns its pasture in 300,000 ticks.
#[test]
#[ignore]
fn a_flock_on_generated_ground_lasts() {
    let seed = server::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT);
    let mut made_world = server::start_world(server::WorldStart { seed, sheep_per_flock: 4_000, ..server::WorldStart::default() });
    for _ in 0..300_000 {
        made_world.tick();
    }
    let grass_cells: u64 = made_world.bitmap_arena().superchunks().iter().map(|superchunk| made_world.bitmap_arena().superchunk_count(GRASS, superchunk.superchunk_index()) as u64).sum();
    let sheep_count = made_world.entity_store().len();
    assert!((500..200_000).contains(&sheep_count), "{sheep_count} sheep");
    assert!(grass_cells > 4 * 1024 * 1024 / 20, "{grass_cells} cells of grass");
}
