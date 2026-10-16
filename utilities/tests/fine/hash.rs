//! Hashing: keys to slots, and words mixed.
//!
//! `cargo test`

use utilities::hash::{mix_word, slot, GOLDEN_RATIO};
use utilities::rng::RandomSource;

/// Consecutive keys land in every slot of a table, each slot in range,
/// and a word mixed is what the random source draws from its state.
#[test]
fn keys_spread_over_every_slot_and_words_mix_as_draws_do() {
    let mut random_numbers = RandomSource::new(utilities::seed::counted_seed());
    // Tables of every size there is a power of two for, the keys starting anywhere.
    for slots in (1..=12).map(|power_of_two| 1usize << power_of_two) {
        let mut slots_hit = vec![false; slots];
        let first_key = random_numbers.next_draw();
        for hashed_key in (0..slots as u64 * 4).map(|key_offset| first_key.wrapping_add(key_offset)) {
            slots_hit[slot(hashed_key, slots)] = true;
        }
        assert!(slots_hit.iter().all(|&slot_hit| slot_hit), "{slots} slots");
    }
    for _ in 0..100 {
        let seed = random_numbers.next_draw();
        assert_eq!(RandomSource::new(seed).next_draw(), mix_word(seed.wrapping_add(GOLDEN_RATIO)), "seed {seed}");
    }
}
