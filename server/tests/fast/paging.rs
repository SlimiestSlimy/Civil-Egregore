//! The cold pool paged to disk: a world keeping none of its cold
//! superchunks' images in memory is the world keeping them all.

use protocol::server_parameters::{CleanServerParameters, ParameterValue, CAMERA_FORCES_SUPERCHUNKS_HOT, COLD_POOL_BYTES, SEED};
use crate::whole_worlds::world_contents;
use utilities::coordinates::{SuperchunkIndex, WORLD_SIDE_SUPERCHUNKS};
use crate::{World, COOL_TICKS, WARM_TICKS};
use protocol::pipe::frame::Viewport;
use utilities::rng::RandomSource;

/// Ticks `paged` and `kept_world` with the viewport on the superchunk `camera_superchunk_cartesian`
/// until the one seen before has gone cold: after each tick what the
/// paged world holds together holds and no hot superchunk's image is
/// on disk. Adds to `been_on_disk` each superchunk whose image is
/// seen there, and says whether one of them was hot again.
fn watch_camera_at(paged: &mut World, kept_world: &mut World, camera_superchunk_cartesian: (u32, u32), been_on_disk: &mut Vec<SuperchunkIndex>) -> bool {
    let (camera_viewport, mut hot_again) = (Some(Viewport { top_left_superchunk_cartesian: camera_superchunk_cartesian, bottom_right_superchunk_cartesian: camera_superchunk_cartesian }), false);
    paged.keep_viewport(camera_viewport);
    kept_world.keep_viewport(camera_viewport);
    for _ in 0..WARM_TICKS.max(COOL_TICKS) + 20 {
        paged.tick();
        kept_world.tick();
        assert_eq!(paged.broken_invariant(), None, "at tick {}", paged.entity_store().current_tick());
        let hot = paged.bitmap_arena().superchunk_indices();
        assert!(hot.iter().all(|&superchunk| !paged.chunk_storage().is_on_disk(superchunk)), "a hot superchunk's image is in memory");
        hot_again |= hot.iter().any(|superchunk| been_on_disk.contains(superchunk));
        let newly_on_disk: Vec<SuperchunkIndex> = paged.chunk_storage().superchunks().filter(|&superchunk| paged.chunk_storage().is_on_disk(superchunk) && !been_on_disk.contains(&superchunk)).collect();
        been_on_disk.extend(newly_on_disk);
    }
    assert_eq!(kept_world.chunk_storage().on_disk(), 0, "nothing paged of a world that keeps its images");
    hot_again
}

/// Whether `paged` and `kept_world` hold the same, every image flushed and
/// those on disk read.
fn the_same(paged: &mut World, kept_world: &mut World) -> bool {
    paged.write_back_and_flush_all();
    kept_world.write_back_and_flush_all();
    world_contents(paged) == world_contents(kept_world)
}

/// Two worlds of one seed, the camera taken over three superchunks
/// in an order drawn, twice, each left to go cold: the one keeping no
/// cold image in memory pages them out, makes them hot again from
/// disk, and is at every stop the world that keeps them all. Saved
/// and loaded keeping none, its images are left in the save and it is
/// that world still; saved again there, the save loads as it; and it
/// goes on as that world.
#[test]
fn a_world_paged_to_disk_is_the_world_kept_in_memory() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let world_start = CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(CAMERA_FORCES_SUPERCHUNKS_HOT, crate::whole_worlds::given());
    let (mut paged, mut kept_world) = (crate::test_game::made(&crate::test_game::in_folder(world_start.clone(), "a_world_paged_to_disk", 0)), crate::test_game::made(&crate::test_game::in_folder(world_start, "a_world_kept_in_memory", 0)));
    paged.keep_cold_pool_within(0);
    let (middle_coordinate, mut order_random) = (WORLD_SIDE_SUPERCHUNKS / 2, RandomSource::new(crate::test_seed::TestSeedHandle::open().seed()));
    // The three places in an order drawn.
    let mut places: Vec<(u32, u32)> = (0..3).map(|step_number| (middle_coordinate + 2 * step_number, middle_coordinate)).collect();
    for last_index in (1..places.len()).rev() {
        places.swap(last_index, order_random.draw_below(last_index as u64 + 1) as usize);
    }
    let (mut been_on_disk, mut hot_again) = (Vec::new(), false);
    for &camera_cartesian in places.iter().chain(&places) {
        hot_again |= watch_camera_at(&mut paged, &mut kept_world, camera_cartesian, &mut been_on_disk);
        assert!(the_same(&mut paged, &mut kept_world), "the same with the camera at {camera_cartesian:?}");
    }
    assert!(been_on_disk.len() >= 3 && hot_again, "{} superchunks paged out, one hot again: {hot_again}", been_on_disk.len());

    crate::save(&mut paged).expect("saved");
    let saved_in = paged.folder().to_path_buf();
    let saved_types = std::sync::Arc::new(crate::read_types(&saved_in, crate::test_game::TestGame::register_types).expect("its types"));
    let mut loaded_world = crate::run_world(&saved_in, &saved_types, &CleanServerParameters::default().with(COLD_POOL_BYTES, ParameterValue::Integer(0))).expect("loaded");
    assert!(loaded_world.chunk_storage().on_disk() > 0, "images left in the save");
    kept_world.keep_viewport(None);
    assert!(the_same(&mut loaded_world, &mut kept_world), "loaded as the world that kept its images");
    crate::save(&mut loaded_world).expect("saved where it was loaded from");
    assert!(the_same(&mut crate::whole_worlds::opened(&saved_in).expect("loaded"), &mut kept_world), "saved again");
    drop(paged);
    for &camera_cartesian_again in &places {
        watch_camera_at(&mut loaded_world, &mut kept_world, camera_cartesian_again, &mut been_on_disk);
        assert!(the_same(&mut loaded_world, &mut kept_world), "the same, loaded, with the camera at {camera_cartesian_again:?}");
    }
}
