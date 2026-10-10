//! The complete tier: larger worlds run for longer -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release --test complete -- --ignored`

mod tests;

use type_registry::GRASS;
use tests::{everything, folder};

/// A world run 30,000 ticks straight, and the same world saved and
/// loaded every 5,000: the same at the end.
#[test]
#[ignore]
fn a_world_stopped_often_comes_to_the_same() {
    const UNTIL: u64 = 30_000;
    let mut straight = server::start(server::Start { seed: 21, sheep: 4_000, ..server::Start::default() });
    while straight.entities.now() < UNTIL {
        straight.tick();
    }
    let folder = folder("complete");
    let mut stopped = server::start(server::Start { seed: 21, sheep: 4_000, ..server::Start::default() });
    for stop in (5_000..=UNTIL).step_by(5_000) {
        while stopped.entities.now() < stop {
            stopped.tick();
        }
        server::save(&folder, &mut stopped).expect("saved");
        stopped = server::load(&folder).expect("loaded");
    }
    // A cooled superchunk's last changes wait in the writeback ring: flushed, as a save's are, before the images are set side by side.
    straight.write_back_and_flush_all();
    assert!(everything(&straight) == everything(&stopped), "the same at tick {UNTIL}");
}

/// A flock on generated ground, walls and all, its halos following it,
/// neither dies out nor overruns its pasture in 300,000 ticks.
#[test]
#[ignore]
fn a_flock_on_generated_ground_lasts() {
    let mut made = server::start(server::Start { seed: 3, sheep: 4_000, ..server::Start::default() });
    for _ in 0..300_000 {
        made.tick();
    }
    let grass: u64 = made.arena.superchunks().iter().map(|superchunk| made.arena.superchunk_count(GRASS, superchunk.index()) as u64).sum();
    let sheep = made.entities.len();
    assert!((500..200_000).contains(&sheep), "{sheep} sheep");
    assert!(grass > 4 * 1024 * 1024 / 20, "{grass} cells of grass");
}
