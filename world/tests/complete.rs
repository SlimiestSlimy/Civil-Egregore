//! The complete tier: larger worlds run for longer -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release --test complete -- --ignored`

use chunk_storage::mock::{DIRT, GRASS};
use simulation::entity_store::{Attribute, Header};
use world::{transient_data, World};

/// Every hot cell, every entity with its attributes, the tick, every
/// random stream, and every cold superchunk's kept state.
type Everything = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, u64, Vec<(coordinates::SuperchunkIndex, u64)>, Vec<Vec<u64>>);

/// [`Everything`] `world` holds.
fn everything(world: &World) -> Everything {
    let cells = [DIRT, GRASS].into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.cells().to_vec()).collect();
    let all = world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    (cells, all, world.entities.now(), world.simulation.random_states().collect(), world.cold.values().cloned().collect())
}

/// A world run 30,000 ticks straight, and the same world saved and
/// loaded every 5,000: the same at the end.
#[test]
#[ignore]
fn a_world_stopped_often_comes_to_the_same() {
    const UNTIL: u64 = 30_000;
    let mut straight = world::generate(21, 4_000);
    while straight.entities.now() < UNTIL {
        straight.tick();
    }
    let folder = transient_data::saves().join("tests").join("complete");
    let _ = std::fs::remove_dir_all(&folder);
    let mut stopped = world::generate(21, 4_000);
    for stop in (5_000..=UNTIL).step_by(5_000) {
        while stopped.entities.now() < stop {
            stopped.tick();
        }
        world::save(&folder, &mut stopped).expect("saved");
        stopped = world::load(&folder).expect("loaded");
    }
    assert!(everything(&straight) == everything(&stopped), "the same at tick {UNTIL}");
}

/// A flock on generated ground, walls and all, its halos following it,
/// neither dies out nor overruns its pasture in 300,000 ticks.
#[test]
#[ignore]
fn a_flock_on_generated_ground_lasts() {
    let mut made = world::generate(3, 4_000);
    for _ in 0..300_000 {
        made.tick();
    }
    let grass: u64 = made.arena.superchunks().iter().map(|superchunk| made.arena.superchunk_count(GRASS, superchunk.index()) as u64).sum();
    let sheep = made.entities.len();
    assert!((500..200_000).contains(&sheep), "{sheep} sheep");
    assert!(grass > 4 * 1024 * 1024 / 20, "{grass} cells of grass");
}
