//! The cold pool paged to disk: a world keeping none of its cold
//! superchunks' images in memory is the world keeping them all.

use crate::tests::{everything, folder, land_seed};
use coordinates::{SuperchunkIndex, WORLD_SIDE_SUPERCHUNKS};
use server::{Start, World, COOL_TICKS, WARM_TICKS};
use simulation::halos::Viewport;
use utilities::rng::Rng;

/// Ticks `paged` and `kept` with the viewport on the superchunk `at`
/// until the one seen before has gone cold: after each tick what the
/// paged world holds together holds and no hot superchunk's image is
/// on disk. Adds to `been_on_disk` each superchunk whose image is
/// seen there, and says whether one of them was hot again.
fn watch(paged: &mut World, kept: &mut World, at: (u32, u32), been_on_disk: &mut Vec<SuperchunkIndex>) -> bool {
    let (view, mut hot_again) = (Some(Viewport { first: at, last: at }), false);
    paged.keep_viewport(view);
    kept.keep_viewport(view);
    for _ in 0..WARM_TICKS.max(COOL_TICKS) + 20 {
        paged.tick();
        kept.tick();
        assert_eq!(paged.broken_invariant(), None, "at tick {}", paged.entities().now());
        let hot = paged.arena().superchunk_indices();
        assert!(hot.iter().all(|&superchunk| !paged.storage().is_on_disk(superchunk)), "a hot superchunk's image is in memory");
        hot_again |= hot.iter().any(|superchunk| been_on_disk.contains(superchunk));
        let newly: Vec<SuperchunkIndex> = paged.storage().superchunks().filter(|&superchunk| paged.storage().is_on_disk(superchunk) && !been_on_disk.contains(&superchunk)).collect();
        been_on_disk.extend(newly);
    }
    assert_eq!(kept.storage().on_disk(), 0, "nothing paged of a world that keeps its images");
    hot_again
}

/// Whether `paged` and `kept` hold the same, every image flushed and
/// those on disk read.
fn the_same(paged: &mut World, kept: &mut World) -> bool {
    paged.write_back_and_flush_all();
    kept.write_back_and_flush_all();
    everything(paged) == everything(kept)
}

/// Two worlds of one seed, the camera taken over three superchunks
/// in an order drawn, twice, each left to go cold: the one keeping no
/// cold image in memory pages them out, makes them hot again from
/// disk, and is at every stop the world that keeps them all. Saved
/// and loaded keeping none, its images are left in the save and it is
/// that world still; saved again there and elsewhere, both saves load
/// as it; and it goes on as that world.
#[test]
fn a_world_paged_to_disk_is_the_world_kept_in_memory() {
    let start = Start { seed: land_seed(8), sheep: 0, camera_loads: true, ..Start::default() };
    let (mut paged, mut kept) = (server::start(start), server::start(start));
    paged.keep_cold_pool_within(0);
    let (middle, mut random) = (WORLD_SIDE_SUPERCHUNKS / 2, Rng::new(utilities::seed::counted()));
    // The three places in an order drawn.
    let mut places: Vec<(u32, u32)> = (0..3).map(|along| (middle + 2 * along, middle)).collect();
    for last in (1..places.len()).rev() {
        places.swap(last, random.below(last as u64 + 1) as usize);
    }
    let (mut been_on_disk, mut hot_again) = (Vec::new(), false);
    for &at in places.iter().chain(&places) {
        hot_again |= watch(&mut paged, &mut kept, at, &mut been_on_disk);
        assert!(the_same(&mut paged, &mut kept), "the same with the camera at {at:?}");
    }
    assert!(been_on_disk.len() >= 3 && hot_again, "{} superchunks paged out, one hot again: {hot_again}", been_on_disk.len());

    let (saved_in, copied_to) = (folder("paging_saved"), folder("paging_copied"));
    server::save(&saved_in, &mut paged).expect("saved");
    let mut loaded = server::load_keeping(&saved_in, 0).expect("loaded");
    assert!(loaded.storage().on_disk() > 0, "images left in the save");
    kept.keep_viewport(None);
    assert!(the_same(&mut loaded, &mut kept), "loaded as the world that kept its images");
    server::save(&saved_in, &mut loaded).expect("saved where it was loaded from");
    server::save(&copied_to, &mut loaded).expect("saved elsewhere");
    for save in [&saved_in, &copied_to] {
        assert!(the_same(&mut server::load(save).expect("loaded"), &mut kept), "the save in {}", save.display());
    }
    drop(paged);
    for &at in &places {
        watch(&mut loaded, &mut kept, at, &mut been_on_disk);
        assert!(the_same(&mut loaded, &mut kept), "the same, loaded, with the camera at {at:?}");
    }
}
