//! Halos: between ticks, the superchunks hot and not cooling, or
//! warming, are exactly the 3x3 about every superchunk a sheep stands
//! in; a superchunk gone cold
//! comes back as it was, to the cell, the entity and the random number.
//!
//! `cargo test`

use basegame::base_types::GRASS;
use utilities::coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use server::bitplane_manager::{Write, WriteOp};
use server::entity_manager::{AttributeBlock, EntityType, Header, NEVER_WAKES};
use protocol::pipe::frame::Viewport;
use server::{halos_about, HaloChange, World, COOL_TICKS, WARM_TICKS};

/// The superchunks holding a hot entity.
fn hot_entities(world: &World) -> Vec<SuperchunkIndex> {
    let hot_entity = server::simulation::hot_entity_of(world.frozen_types()).expect("a type keeps the world hot");
    world.entity_store().superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| entity.header.entity_type == hot_entity)).map(|superchunk| superchunk.superchunk_index()).collect()
}

/// A flock wandering off its origin for 6,000 ticks, or as many as it takes a halo to move: after every tick
/// the superchunks hot and not cooling, or warming, are the halos about
/// the sheep, never both; every one warming turns hot, wanted still or not,
/// and every one cooling cold, when due; every superchunk ever made is hot or cold and
/// never both; and the world has grown.
#[test]
fn the_hot_superchunks_are_the_halos() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 4_000);
    assert_eq!(world.bitmap_arena().superchunk_indices(), halos_about([WORLD_MIDDLE].into_iter()), "the origin's halo");
    let mut halos_moved = HaloChange::default();
    // 6,000 ticks at least, and on until a superchunk has been generated: on some seeds the flock is long in nearing an edge.
    while world.entity_store().current_tick() < 6_000 || halos_moved.superchunks_generated == 0 {
        assert!(world.entity_store().current_tick() < 60_000, "the halos moved");
        halos_moved += world.tick().halos;
        let hot = world.bitmap_arena().superchunk_indices();
        let warming: Vec<(SuperchunkIndex, u64)> = world.warming().collect();
        let cooling: Vec<(SuperchunkIndex, u64)> = world.cooling().collect();
        assert!(cooling.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_ok()), "cooling, hot still");
        let staying_hot = hot.iter().filter(|superchunk| cooling.binary_search_by_key(superchunk, |(cooling, _)| cooling).is_err());
        // A warming is never given up: one its halo has left is warming still, and no halo's.
        let wanted_halos = halos_about(hot_entities(&world).into_iter());
        let mut halos: Vec<SuperchunkIndex> = staying_hot.copied().chain(warming.iter().map(|&(superchunk, _)| superchunk).filter(|superchunk| wanted_halos.binary_search(superchunk).is_ok())).collect();
        halos.sort_unstable();
        assert!(warming.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_err()), "warming, not hot yet");
        assert_eq!(halos, wanted_halos, "hot and not cooling, or warming, never both");
        let tick_now = world.entity_store().current_tick();
        let due_within = |due_list: &[(SuperchunkIndex, u64)], ticks: u64| due_list.iter().all(|&(_, due_tick)| due_tick > tick_now && due_tick <= tick_now + ticks);
        assert!(due_within(&warming, WARM_TICKS) && due_within(&cooling, COOL_TICKS), "each due within its time");
        assert!(hot.iter().all(|superchunk| !world.cold().contains_key(superchunk)), "hot or cold, never both");
        assert_eq!(world.chunk_storage().superchunks().count(), hot.len() + world.cold().len(), "every superchunk made, hot or cold");
    }
    // Every one reached turned hot, or is warming still: none is given up.
    assert!(halos_moved.superchunks_generated > 0 && halos_moved.superchunks_reached == halos_moved.superchunks_generated + halos_moved.superchunks_restored + world.warming().count(), "the halos moved, every one reached made hot: {halos_moved:?}");
}

/// A superchunk a halo reaches is warming for its time: a write to it is
/// missed and an entity put there lost, until it turns hot at the tick
/// it is due -- still wanted or not -- and then both hold.
#[test]
fn a_superchunk_warming_takes_nothing_until_it_turns_hot() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 4_000);
    let try_both = |world: &mut World, cell| {
        let missed = world.write_cells(GRASS, [Write::cell(cell, WriteOp::FlipCell)]).missed;
        (missed, world.add_entity(Header { entity_type: EntityType::from_number(99), stood_cell: cell, wake: NEVER_WAKES }, &[]).lost)
    };
    while world.warming().next().is_none() {
        assert!(world.entity_store().current_tick() < 60_000, "the halos moved");
        world.tick();
    }
    let (superchunk, due_tick) = world.warming().next().expect("one warming");
    let cell = superchunk.chunks().next().expect("a chunk").top_left();
    assert_eq!(try_both(&mut world, cell), (1, 1), "warming: the write missed, the entity lost");
    while world.entity_store().current_tick() < due_tick {
        assert!(world.bitmap_arena().superchunk_indices().binary_search(&superchunk).is_err(), "not hot before it is due");
        world.tick();
    }
    assert!(world.bitmap_arena().superchunk_indices().binary_search(&superchunk).is_ok(), "hot when due, its halo there still or gone");
    assert_eq!(try_both(&mut world, cell), (0, 0), "hot: both hold");
}

/// A lone sheep taken away: its halo is cooling, hot still, for its
/// time; the sheep back before it is due, the halo stays hot; taken away
/// again, cooling across a save and a load, it goes cold when due.
#[test]
fn a_superchunk_cooling_stays_hot_until_due() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 1);
    let halo = world.bitmap_arena().superchunk_indices();
    let lone_sheep = world.entity_store().iter().map(|entity| (entity.header, entity.attributes.to_vec())).next().expect("the sheep");
    let take_away = |world: &mut World| {
        crate::worlds::remove_entity(world, &lone_sheep.0);
        world.tick().halos
    };
    let cooling = |world: &World| world.cooling().map(|(superchunk, _)| superchunk).collect::<Vec<_>>();

    assert_eq!(take_away(&mut world).superchunks_cooled, 0, "none cold at once");
    let first_due_tick = world.entity_store().current_tick() + COOL_TICKS;
    assert!(cooling(&world) == halo && world.cooling().all(|(_, cold_at_tick)| cold_at_tick == first_due_tick), "the halo cooling, due {COOL_TICKS} on");
    while world.entity_store().current_tick() < first_due_tick - COOL_TICKS / 2 {
        world.tick();
    }
    // Back asleep over the tick that follows: awake, it might step over an edge and move its halo.
    let wake = lone_sheep.0.wake.max(world.entity_store().current_tick() + 2);
    crate::worlds::add_entity(&mut world, Header { wake, ..lone_sheep.0 }, &lone_sheep.1);
    let back_change = world.tick().halos;
    assert!(back_change == HaloChange::default() && world.cooling().next().is_none() && world.bitmap_arena().superchunk_indices() == halo, "the sheep back: the halo hot as it was, nothing made");

    take_away(&mut world);
    let second_due_tick = world.entity_store().current_tick() + COOL_TICKS;
    let folder = crate::worlds::folder("cooling");
    server::save(&folder, &mut world).expect("saved");
    world = server::load_world(&folder, &crate::worlds::types()).expect("loaded");
    assert!(cooling(&world) == halo && world.cooling().all(|(_, goes_cold_tick)| goes_cold_tick == second_due_tick), "cooling as it was");
    let mut cooled_count = 0;
    while world.entity_store().current_tick() < second_due_tick {
        assert_eq!(world.bitmap_arena().superchunk_indices(), halo, "hot until due");
        cooled_count += world.tick().halos.superchunks_cooled;
    }
    assert!(cooled_count == halo.len() && world.bitmap_arena().superchunk_indices().is_empty() && world.cooling().next().is_none(), "all cold when due");
}

/// Every superchunk made cold, then the origin's halo hot again: its
/// cells, its sheep and their attributes, and its random numbers are as
/// they were -- and it ticks on as it would have. Once its bitmaps
/// lingering are kept; once flushed and let go, so decoded from its images.
#[test]
fn a_superchunk_gone_cold_comes_back_as_it_was() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 2_000);
    // 500 ticks, and on to one with nothing warming or cooling: made cold and hot again by hand, one warming would not turn hot when it was due.
    while world.entity_store().current_tick() < 500 || world.warming().next().is_some() || world.cooling().next().is_some() {
        assert!(world.entity_store().current_tick() < 60_000, "the halos at rest");
        world.tick();
    }
    let ticks_run = world.entity_store().current_tick();
    let halo = world.bitmap_arena().superchunk_indices();
    type WorldHeld = (Vec<u64>, Vec<(Header, Vec<AttributeBlock>)>, Vec<(SuperchunkIndex, u64)>);
    let world_held = |world: &World| -> WorldHeld {
        let cells = world.world_info().layers.clone().into_iter().flat_map(|layer| world.bitmap_arena().buckets_of(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
        (cells, world.entity_store().iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect(), world.world_simulation().random_states().collect())
    };
    let held_before = world_held(&world);
    let mut twin_world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 2_000);
    for _ in 0..ticks_run {
        twin_world.tick();
    }

    let cooled_change = world.keep_hot(&[]);
    assert_eq!((cooled_change.superchunks_cooled, world.bitmap_arena().superchunk_indices().len(), world.entity_store().len()), (halo.len(), 0, 0), "all cold");
    // Those the halo left on its way here are cold too, on a seed whose sheep walked it off them.
    assert!(halo.iter().all(|superchunk| world.cold().contains_key(superchunk)), "every one of the halo's kept cold");
    let restored_change = world.keep_hot(&halo);
    assert_eq!((restored_change.superchunks_restored, restored_change.superchunks_generated), (halo.len(), 0), "restored as they were, none generated");
    assert!(world_held(&world) == held_before, "as it was");

    for _ in 0..500 {
        world.tick();
        twin_world.tick();
    }
    assert!(world_held(&world) == world_held(&twin_world), "ticks on as it would have");

    // On to a tick with nothing warming or cooling: on some seeds a sheep is by an edge just now.
    let halos_unsettled = |world: &World| world.warming().next().is_some() || world.cooling().next().is_some();
    for _ in 0..20_000 {
        if !halos_unsettled(&world) {
            break;
        }
        world.tick();
        twin_world.tick();
    }
    // Cold again, and saved: every change flushed into the images, the bitmaps lingering let go; made hot, decoded from those images.
    let halos = world.bitmap_arena().superchunk_indices();
    assert!(world.warming().next().is_none() && world.cooling().next().is_none() && halos == twin_world.bitmap_arena().superchunk_indices(), "the two alike, none warming or cooling");
    world.keep_hot(&[]);
    server::save(&crate::worlds::folder("gone_cold"), &mut world).expect("saved");
    assert_eq!(world.bitmap_arena().lingering(), 0, "flushed, so let go");
    world.keep_hot(&halos);
    for _ in 0..500 {
        world.tick();
        twin_world.tick();
    }
    assert!(world_held(&world) == world_held(&twin_world), "decoded as it was, and ticks on as it would have");
}

/// A world of a size has sheep on every superchunk of it -- its
/// generation gives each a flock the first tick it is hot, the origin's
/// first, then those its flock's halo generates, and on to its side --
/// and is never hot outside it, whoever wants it and whatever the
/// flocks do; a save keeps its size. Forced hot, all of it is hot with no
/// sheep at all, and stays so, saved and loaded.
#[test]
fn a_world_of_a_size_is_hot_within_it_only() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let side_superchunks = 4;
    let world_size = server::WorldSize::SquareOfSide { side_superchunks, forced_hot: false };
    let mut world = crate::worlds::start_world(server::WorldStart { seed, world_size, generation_numbers: crate::worlds::flocked(500), ..server::WorldStart::default() });
    while world.bitmap_arena().superchunk_indices().len() < (side_superchunks * side_superchunks) as usize || !world.unpopulated().is_empty() {
        assert!(world.entity_store().current_tick() < 8 * WARM_TICKS, "the square hot and populated, flock by flock");
        world.tick();
    }
    let hot = world.bitmap_arena().superchunk_indices();
    assert!(hot.len() == (side_superchunks * side_superchunks) as usize && hot.iter().all(|&superchunk| world.halos().hot.is_within_world(superchunk)), "sheep everywhere in it: all of it hot, nothing outside it");
    assert!(hot.iter().all(|&superchunk| world.entity_store().superchunk(superchunk).is_some_and(|superchunk_entities| !superchunk_entities.is_empty())), "sheep on every superchunk");
    world.keep_hot(&halos_about(hot.into_iter()));
    assert_eq!(world.bitmap_arena().superchunk_indices().len(), (side_superchunks * side_superchunks) as usize, "nothing made hot outside it, whoever wants it");
    for _ in 0..2 * WARM_TICKS {
        world.tick();
        assert!(world.bitmap_arena().superchunk_indices().into_iter().chain(world.warming().map(|(superchunk, _)| superchunk)).all(|superchunk| world.halos().hot.is_within_world(superchunk)));
    }
    let folder = crate::worlds::folder("sized");
    server::save(&folder, &mut world).expect("saved");
    assert_eq!(server::load_world(&folder, &crate::worlds::types()).expect("loaded").halos().hot, world.halos().hot, "its size, kept");

    let forced_size = server::WorldSize::SquareOfSide { side_superchunks, forced_hot: true };
    let mut world = crate::worlds::start_world(server::WorldStart { seed, world_size: forced_size, generation_numbers: crate::worlds::flocked(0), ..server::WorldStart::default() });
    assert_eq!(world.bitmap_arena().superchunk_indices().len(), (side_superchunks * side_superchunks) as usize, "forced hot: all of it, no sheep needed");
    _ = std::fs::remove_dir_all(&folder);
    server::save(&folder, &mut world).expect("saved");
    let mut loaded_world = server::load_world(&folder, &crate::worlds::types()).expect("loaded");
    assert_eq!(loaded_world.halos().hot, world.halos().hot, "forced, kept");
    for _ in 0..=WARM_TICKS {
        loaded_world.tick();
    }
    assert_eq!(loaded_world.bitmap_arena().superchunk_indices().len(), (side_superchunks * side_superchunks) as usize, "forced hot: all of it, all the while");
    assert_eq!((loaded_world.warming().count(), loaded_world.cooling().count()), (0, 0));
}

/// A world whose camera loads superchunks: a wide viewport is wanted
/// hot whole, every superchunk of it, and no viewport none; two
/// superchunks of the viewport, far from the origin, turn hot as a
/// halo's would, generated and unpopulated until their first tick hot.
/// A save keeps that its camera loads, and those generated yet
/// unpopulated; a world whose camera loads nothing keeps no viewport.
/// No sheep: on a world with no end, flocks would spread it for good.
#[test]
fn the_camera_loads_the_superchunks_of_the_viewport() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::start_world(server::WorldStart { seed, generation_numbers: crate::worlds::flocked(0), camera_loads: true, ..server::WorldStart::default() });
    assert!(world.world_info().camera_loads);
    let (x, y) = WORLD_MIDDLE.cartesian();
    world.keep_viewport(Some(Viewport { top_left_superchunk_cartesian: (x - 50, y - 50), bottom_right_superchunk_cartesian: (x + 50, y + 50) }));
    let wide_viewport = world.halos().viewport();
    assert!(wide_viewport.len() == 101 * 101 && wide_viewport.contains(&WORLD_MIDDLE), "{} in the viewport", wide_viewport.len());
    world.keep_viewport(None);
    assert!(world.halos().viewport().is_empty(), "no viewport, nothing kept for it");
    world.keep_viewport(Some(Viewport { top_left_superchunk_cartesian: (x + 10, y), bottom_right_superchunk_cartesian: (x + 11, y) }));
    let viewport = [SuperchunkIndex::from_cartesian(x + 10, y), SuperchunkIndex::from_cartesian(x + 11, y)];
    while !viewport.iter().all(|superchunk| world.bitmap_arena().superchunk_indices().contains(superchunk)) {
        assert!(world.entity_store().current_tick() < 4 * WARM_TICKS, "the viewport generated and hot");
        world.tick();
    }
    assert!(viewport.iter().all(|superchunk| world.unpopulated().contains(&superchunk.0)), "generated, and unpopulated until its first tick hot");
    let folder = crate::worlds::folder("camera");
    server::save(&folder, &mut world).expect("saved");
    let loaded_world = server::load_world(&folder, &crate::worlds::types()).expect("loaded");
    assert!(loaded_world.world_info().camera_loads, "that its camera loads, kept");
    assert_eq!(loaded_world.unpopulated(), world.unpopulated(), "those unpopulated, kept");
    world.tick();
    assert!(viewport.iter().all(|superchunk| !world.unpopulated().contains(&superchunk.0)), "populated on its first tick hot");

    let mut unseen_world = crate::worlds::start_world(server::WorldStart { seed, generation_numbers: crate::worlds::flocked(0), ..server::WorldStart::default() });
    unseen_world.keep_viewport(Some(Viewport { top_left_superchunk_cartesian: (x + 10, y), bottom_right_superchunk_cartesian: (x + 11, y) }));
    assert!(unseen_world.halos().viewport().is_empty() && !unseen_world.world_info().camera_loads, "its camera loads nothing");
}
