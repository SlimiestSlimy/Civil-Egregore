//! Hashing: keys to slots, and words mixed.
//!
//! `cargo test`

use utilities::hash::{mix, slot, GOLDEN_RATIO};
use utilities::rng::Rng;

/// Consecutive keys land in every slot of a table, each slot in range,
/// and a word mixed is what the random source draws from its state.
#[test]
fn keys_spread_over_every_slot_and_words_mix_as_draws_do() {
    let mut random = Rng::new(utilities::seed::counted());
    // Tables of every size there is a power of two for, the keys starting anywhere.
    for slots in (1..=12).map(|power| 1usize << power) {
        let mut hit = vec![false; slots];
        let first = random.draw();
        for key in (0..slots as u64 * 4).map(|nth| first.wrapping_add(nth)) {
            hit[slot(key, slots)] = true;
        }
        assert!(hit.iter().all(|&hit| hit), "{slots} slots");
    }
    for _ in 0..100 {
        let seed = random.draw();
        assert_eq!(Rng::new(seed).draw(), mix(seed.wrapping_add(GOLDEN_RATIO)), "seed {seed}");
    }
}
