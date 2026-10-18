//! A world's hash: the same for the same world however it was ticked
//! -- on one thread or several, hashed along the way or not -- and
//! another for a world a tick on.
//!
//! `cargo test`

use protocol::server_parameters::{CleanServerParameters, ParameterValue, SEED, THREADS};
use server::world_hash;

/// A world ticked on one thread and hashed every so often, and the
/// same world ticked on several and hashed once: at every tick both
/// are hashed the hashes are the same, part by part, and no two ticks
/// of one world hash the same.
#[test]
fn the_same_world_hashes_the_same_however_it_is_ticked() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut one_thread_world = crate::worlds::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(THREADS, ParameterValue::Integer(1)), 2_000);
    let mut several_threads_world = crate::worlds::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(THREADS, ParameterValue::Integer(3)), 2_000);
    let mut hashes_seen = vec![world_hash(&mut one_thread_world).whole_hash(seed)];
    for stop_number in 1..=6 {
        for _ in 0..100 {
            one_thread_world.tick();
            several_threads_world.tick();
        }
        let hash_by_part = world_hash(&mut one_thread_world);
        if stop_number % 3 == 0 {
            assert_eq!(hash_by_part, world_hash(&mut several_threads_world), "at tick {}", hash_by_part.tick);
        }
        assert!(!hashes_seen.contains(&hash_by_part.whole_hash(seed)), "tick {} hashed as one before it", hash_by_part.tick);
        hashes_seen.push(hash_by_part.whole_hash(seed));
    }
    assert_ne!(world_hash(&mut one_thread_world).whole_hash(seed), world_hash(&mut one_thread_world).whole_hash(seed ^ 1), "the seed is part of the hash");
}
