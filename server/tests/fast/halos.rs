//! Halos: between ticks, the superchunks hot and not cooling, or
//! warming, are exactly the 3x3 about every superchunk a sheep stands
//! in; a superchunk gone cold
//! comes back as it was, to the cell, the entity and the random number.
//!
//! `cargo test`

use chunk_storage::mock::GRASS;
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use bitplane_manager::{Write, WriteOp};
use simulation::entity_store::{Attribute, EntityId, EntityType, Header, NEVER};
use server::halos::about;
use server::{HaloChange, World, COOL_TICKS, HALO_KEEPERS, WARM_TICKS};

/// The superchunks holding an entity that keeps a halo.
fn keepers(world: &World) -> Vec<SuperchunkIndex> {
    world.entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| HALO_KEEPERS.contains(&entity.header.kind))).map(|superchunk| superchunk.index()).collect()
}

/// A flock wandering off its origin for 6,000 ticks, or as many as it takes a halo to move: after every tick
/// the superchunks hot and not cooling, or warming, are the halos about
/// the sheep, never both; every one warming turns hot, wanted still or not,
/// and every one cooling cold, when due; every superchunk ever made is hot or cold and
/// never both; and the world has grown.
#[test]
fn the_hot_superchunks_are_the_halos() {
    let mut world = server::generate(crate::land_seed(1), 4_000);
    assert_eq!(world.arena.superchunk_indices(), about([WORLD_MIDDLE].into_iter()), "the origin's halo");
    let mut moved = HaloChange::default();
    // 6,000 ticks at least, and on until a superchunk has been generated: on some seeds the flock is long in nearing an edge.
    while world.entities.now() < 6_000 || moved.generated == 0 {
        assert!(world.entities.now() < 60_000, "the halos moved");
        moved += world.tick().halos;
        let hot = world.arena.superchunk_indices();
        let warming: Vec<(SuperchunkIndex, u64)> = world.warming().collect();
        let cooling: Vec<(SuperchunkIndex, u64)> = world.cooling().collect();
        assert!(cooling.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_ok()), "cooling, hot still");
        let staying = hot.iter().filter(|superchunk| cooling.binary_search_by_key(superchunk, |(cooling, _)| cooling).is_err());
        // A warming is never given up: one its halo has left is warming still, and no halo's.
        let wanted = about(keepers(&world).into_iter());
        let mut halos: Vec<SuperchunkIndex> = staying.copied().chain(warming.iter().map(|&(superchunk, _)| superchunk).filter(|superchunk| wanted.binary_search(superchunk).is_ok())).collect();
        halos.sort_unstable();
        assert!(warming.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_err()), "warming, not hot yet");
        assert_eq!(halos, wanted, "hot and not cooling, or warming, never both");
        let now = world.entities.now();
        let within = |due: &[(SuperchunkIndex, u64)], ticks: u64| due.iter().all(|&(_, due)| due > now && due <= now + ticks);
        assert!(within(&warming, WARM_TICKS) && within(&cooling, COOL_TICKS), "each due within its time");
        assert!(hot.iter().all(|superchunk| !world.cold.contains_key(superchunk)), "hot or cold, never both");
        assert_eq!(world.storage.superchunks().count(), hot.len() + world.cold.len(), "every superchunk made, hot or cold");
    }
    // Every one reached turned hot, or is warming still: none is given up.
    assert!(moved.generated > 0 && moved.reached == moved.generated + moved.restored + world.warming().count(), "the halos moved, every one reached made hot: {moved:?}");
}

/// A superchunk a halo reaches is warming for its time: a write to it is
/// missed and an entity put there lost, until it turns hot at the tick
/// it is due -- still wanted or not -- and then both hold.
#[test]
fn a_superchunk_warming_takes_nothing_until_it_turns_hot() {
    let mut world = server::generate(crate::land_seed(1), 4_000);
    let try_both = |world: &mut World, cell| {
        world.arena.queue(GRASS, Write::cell(cell, WriteOp::Flip));
        let missed = world.arena.apply().missed;
        world.entities.queue_put(Header { id: EntityId(u64::MAX), kind: EntityType(99), at: cell, wake: NEVER }, &[]);
        (missed, world.entities.apply().lost)
    };
    while world.warming().next().is_none() {
        assert!(world.entities.now() < 60_000, "the halos moved");
        world.tick();
    }
    let (superchunk, due) = world.warming().next().expect("one warming");
    let cell = superchunk.chunks().next().expect("a chunk").top_left();
    assert_eq!(try_both(&mut world, cell), (1, 1), "warming: the write missed, the entity lost");
    while world.entities.now() < due {
        assert!(world.arena.superchunk_indices().binary_search(&superchunk).is_err(), "not hot before it is due");
        world.tick();
    }
    assert!(world.arena.superchunk_indices().binary_search(&superchunk).is_ok(), "hot when due, its halo there still or gone");
    assert_eq!(try_both(&mut world, cell), (0, 0), "hot: both hold");
}

/// A lone sheep taken away: its halo is cooling, hot still, for its
/// time; the sheep back before it is due, the halo stays hot; taken away
/// again, cooling across a save and a load, it goes cold when due.
#[test]
fn a_superchunk_cooling_stays_hot_until_due() {
    let mut world = server::generate(crate::land_seed(1), 1);
    let halo = world.arena.superchunk_indices();
    let sheep = world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).next().expect("the sheep");
    let take_away = |world: &mut World| {
        world.entities.queue_remove(&sheep.0);
        world.entities.apply();
        world.tick().halos
    };
    let cooling = |world: &World| world.cooling().map(|(superchunk, _)| superchunk).collect::<Vec<_>>();

    assert_eq!(take_away(&mut world).cooled, 0, "none cold at once");
    let due = world.entities.now() + COOL_TICKS;
    assert!(cooling(&world) == halo && world.cooling().all(|(_, at)| at == due), "the halo cooling, due {COOL_TICKS} on");
    while world.entities.now() < due - COOL_TICKS / 2 {
        world.tick();
    }
    // Back asleep over the tick that follows: awake, it might step over an edge and move its halo.
    world.entities.queue_put(Header { wake: sheep.0.wake.max(world.entities.now() + 2), ..sheep.0 }, &sheep.1);
    world.entities.apply();
    let back = world.tick().halos;
    assert!(back == HaloChange::default() && world.cooling().next().is_none() && world.arena.superchunk_indices() == halo, "the sheep back: the halo hot as it was, nothing made");

    take_away(&mut world);
    let due = world.entities.now() + COOL_TICKS;
    let folder = server::transient_data::saves().join("tests").join("cooling");
    server::save(&folder, &mut world).expect("saved");
    world = server::load(&folder).expect("loaded");
    assert!(cooling(&world) == halo && world.cooling().all(|(_, at)| at == due), "cooling as it was");
    let mut cooled = 0;
    while world.entities.now() < due {
        assert_eq!(world.arena.superchunk_indices(), halo, "hot until due");
        cooled += world.tick().halos.cooled;
    }
    assert!(cooled == halo.len() && world.arena.superchunk_indices().is_empty() && world.cooling().next().is_none(), "all cold when due");
}

/// Every superchunk made cold, then the origin's halo hot again: its
/// cells, its sheep and their attributes, and its random numbers are as
/// they were -- and it ticks on as it would have. Once its bitmaps
/// lingering are kept; once flushed and let go, so decoded from its images.
#[test]
fn a_superchunk_gone_cold_comes_back_as_it_was() {
    let mut world = server::generate(crate::land_seed(2), 2_000);
    for _ in 0..500 {
        world.tick();
    }
    let halo = world.arena.superchunk_indices();
    type Held = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, Vec<(SuperchunkIndex, u64)>);
    let held = |world: &World| -> Held {
        let cells = world.info.layers.clone().into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
        (cells, world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect(), world.simulation.random_states().collect())
    };
    let before = held(&world);
    let mut twin = server::generate(crate::land_seed(2), 2_000);
    for _ in 0..500 {
        twin.tick();
    }

    let cooled = world.keep_hot(&[]);
    assert_eq!((cooled.cooled, world.arena.superchunk_indices().len(), world.entities.len()), (halo.len(), 0, 0), "all cold");
    assert_eq!(world.cold.len(), halo.len());
    let restored = world.keep_hot(&halo);
    assert_eq!((restored.restored, restored.generated), (halo.len(), 0), "restored as they were, none generated");
    assert!(held(&world) == before, "as it was");

    for _ in 0..500 {
        world.tick();
        twin.tick();
    }
    assert!(held(&world) == held(&twin), "ticks on as it would have");

    // On to a tick with nothing warming or cooling: on some seeds a sheep is by an edge just now.
    let unsettled = |world: &World| world.warming().next().is_some() || world.cooling().next().is_some();
    for _ in 0..20_000 {
        if !unsettled(&world) {
            break;
        }
        world.tick();
        twin.tick();
    }
    // Cold again, and saved: every change flushed into the images, the bitmaps lingering let go; made hot, decoded from those images.
    let halos = world.arena.superchunk_indices();
    assert!(world.warming().next().is_none() && world.cooling().next().is_none() && halos == twin.arena.superchunk_indices(), "the two alike, none warming or cooling");
    world.keep_hot(&[]);
    server::save(&server::transient_data::saves().join("tests").join("gone_cold"), &mut world).expect("saved");
    assert_eq!(world.arena.lingering(), 0, "flushed, so let go");
    world.keep_hot(&halos);
    for _ in 0..500 {
        world.tick();
        twin.tick();
    }
    assert!(held(&world) == held(&twin), "decoded as it was, and ticks on as it would have");
}
