//! The fixed-capacity list.
//!
//! `cargo test`

use utilities::fixed_list::FixedList;

/// Items come back in order, clearing keeps the room, and pushing
/// past the capacity panics.
#[test]
fn holds_up_to_its_capacity() {
    let mut three_item_list: FixedList<u8, 3> = FixedList::new();
    for pushed_item in [1, 2, 3] {
        three_item_list.push(pushed_item);
    }
    assert_eq!(&*three_item_list, &[1, 2, 3]);
    assert_eq!(three_item_list.pop(), Some(3));
    three_item_list.clear();
    assert!(three_item_list.is_empty());
    for refill_item in [4, 5, 6] {
        three_item_list.push(refill_item);
    }
    assert!(std::panic::catch_unwind(move || three_item_list.push(7)).is_err());
}

/// Pushed to, popped and cleared in an order drawn, a list holds what
/// a `Vec` put through the same holds, at every step.
#[test]
fn a_list_is_a_vec_of_at_most_its_capacity() {
    let mut random_numbers = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    let (mut tested_list, mut model_vec): (FixedList<u64, 12>, Vec<u64>) = (FixedList::new(), Vec::new());
    for step in 0..5_000 {
        match random_numbers.draw_below(8) {
            0..=4 if model_vec.len() < 12 => {
                let drawn_item = random_numbers.next_draw();
                tested_list.push(drawn_item);
                model_vec.push(drawn_item);
            }
            5 | 6 => assert_eq!(tested_list.pop(), model_vec.pop(), "step {step}"),
            7 if random_numbers.draw_below(8) == 0 => {
                tested_list.clear();
                model_vec.clear();
            }
            _ => {}
        }
        assert_eq!((&*tested_list, tested_list.is_empty()), (&model_vec[..], model_vec.is_empty()), "step {step}");
    }
}
