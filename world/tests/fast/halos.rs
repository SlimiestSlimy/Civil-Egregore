//! Halos: the hot superchunks are, between ticks, exactly the 3x3 about
//! every superchunk a sheep stands in; a superchunk gone cold comes back
//! as it was, to the cell, the entity and the random number.
//!
//! `cargo test`

use chunk_storage::mock::{DIRT, GRASS};
use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use simulation::entity_store::{Attribute, Header};
use world::halos::about;
use world::{HaloChange, World, HALO_KEEPERS};

/// The superchunks holding an entity that keeps a halo.
fn keepers(world: &World) -> Vec<SuperchunkIndex> {
    world.entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| HALO_KEEPERS.contains(&entity.header.kind))).map(|superchunk| superchunk.index()).collect()
}

/// A flock wandering off its origin for 6,000 ticks: after every tick
/// the hot superchunks are the halos about the sheep, every superchunk
/// ever made is hot or cold and never both, and the world has grown.
#[test]
fn the_hot_superchunks_are_the_halos() {
    let mut world = world::generate(4, 4_000);
    assert_eq!(world.arena.superchunk_indices(), about([WORLD_MIDDLE].into_iter()), "the origin's halo");
    let mut moved = HaloChange::default();
    for _ in 0..6_000 {
        moved += world.tick().halos;
        let hot = world.arena.superchunk_indices();
        assert_eq!(hot, about(keepers(&world).into_iter()));
        assert!(hot.iter().all(|superchunk| !world.cold.contains_key(superchunk)), "hot or cold, never both");
        assert_eq!(world.storage.superchunks().count(), hot.len() + world.cold.len(), "every superchunk made, hot or cold");
    }
    assert!(moved.generated > 0, "the halos moved: {moved:?}");
}

/// Every superchunk made cold, then the origin's halo hot again: its
/// cells, its sheep and their attributes, and its random numbers are as
/// they were -- and it ticks on as it would have.
#[test]
fn a_superchunk_gone_cold_comes_back_as_it_was() {
    let mut world = world::generate(8, 2_000);
    for _ in 0..500 {
        world.tick();
    }
    let halo = world.arena.superchunk_indices();
    type Held = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, Vec<(SuperchunkIndex, u64)>);
    let held = |world: &World| -> Held {
        let cells = [DIRT, GRASS].into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.cells().to_vec()).collect();
        (cells, world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect(), world.simulation.random_states().collect())
    };
    let before = held(&world);
    let mut twin = world::generate(8, 2_000);
    for _ in 0..500 {
        twin.tick();
    }

    let cooled = world.keep_hot(&[]);
    assert_eq!((cooled.cooled, world.arena.superchunk_indices().len(), world.entities.len()), (halo.len(), 0, 0), "all cold");
    assert_eq!(world.cold.len(), halo.len());
    let warmed = world.keep_hot(&halo);
    assert_eq!((warmed.warmed, warmed.generated), (halo.len(), 0), "warmed from storage, none generated");
    assert!(held(&world) == before, "as it was");

    for _ in 0..500 {
        world.tick();
        twin.tick();
    }
    assert!(held(&world) == held(&twin), "ticks on as it would have");
}
