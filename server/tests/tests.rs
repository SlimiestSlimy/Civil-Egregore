//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use chunk_storage::SuperchunkImage;
use coordinates::SuperchunkIndex;
use entity_manager::{Attribute, Header};
use server::{transient_data, World};
use std::path::PathBuf;

/// A folder of its own for the test `name`, emptied.
pub fn folder(name: &str) -> PathBuf {
    let folder = transient_data::saves().join("tests").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Every hot cell of grass and dirt, every entity with its attributes,
/// the tick, every random stream, every cold superchunk's image and
/// kept state, and the superchunks warming and cooling with when each is due: what
/// two worlds the same hold the same.
pub type Everything = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`Everything`] `world` holds.
pub fn everything(world: &World) -> Everything {
    let cells = world.info.layers.clone().into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let all = world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    let cold = world.cold.iter().map(|(&superchunk, words)| (superchunk, world.storage.image(superchunk).expect("a cold superchunk's image").clone(), words.clone())).collect();
    (cells, all, world.entities.now(), world.simulation.random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// A seed whose world has land about its origin, the `nth` such the
/// run's tests ask for: found from the crate's seed, rolled every few
/// runs (`utilities::seed`), so that nothing passes on one seed alone.
pub fn land_seed(nth: u64) -> u64 {
    worldgen::seed_with_land(utilities::seed::counted().wrapping_add(nth.wrapping_mul(1_000_003)), &worldgen::Shape::DEFAULT, coordinates::WORLD_MIDDLE)
}
