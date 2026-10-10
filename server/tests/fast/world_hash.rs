//! A world's hash: the same for the same world however it was ticked
//! -- on one thread or several, hashed along the way or not -- and
//! another for a world a tick on.
//!
//! `cargo test`

use server::{world_hash, Start};

/// A world ticked on one thread and hashed every so often, and the
/// same world ticked on several and hashed once: at every tick both
/// are hashed the hashes are the same, part by part, and no two ticks
/// of one world hash the same.
#[test]
fn the_same_world_hashes_the_same_however_it_is_ticked() {
    let seed = crate::tests::land_seed(0);
    let mut one = server::start(Start { seed, sheep: 2_000, threads: Some(1), ..Start::default() });
    let mut several = server::start(Start { seed, sheep: 2_000, threads: Some(3), ..Start::default() });
    let mut seen = vec![world_hash(&mut one).whole(seed)];
    for stop in 1..=6 {
        for _ in 0..100 {
            one.tick();
            several.tick();
        }
        let hash = world_hash(&mut one);
        if stop % 3 == 0 {
            assert_eq!(hash, world_hash(&mut several), "at tick {}", hash.tick);
        }
        assert!(!seen.contains(&hash.whole(seed)), "tick {} hashed as one before it", hash.tick);
        seen.push(hash.whole(seed));
    }
    assert_ne!(world_hash(&mut one).whole(seed), world_hash(&mut one).whole(seed ^ 1), "the seed is part of the hash");
}
