//! The fixed-capacity list.
//!
//! `cargo test`

use utilities::fixed_list::FixedList;

/// Items come back in order, clearing keeps the room, and pushing
/// past the capacity panics.
#[test]
fn holds_up_to_its_capacity() {
    let mut list: FixedList<u8, 3> = FixedList::new();
    for item in [1, 2, 3] {
        list.push(item);
    }
    assert_eq!(&*list, &[1, 2, 3]);
    assert_eq!(list.pop(), Some(3));
    list.clear();
    assert!(list.is_empty());
    for item in [4, 5, 6] {
        list.push(item);
    }
    assert!(std::panic::catch_unwind(move || list.push(7)).is_err());
}

/// Pushed to, popped and cleared in an order drawn, a list holds what
/// a `Vec` put through the same holds, at every step.
#[test]
fn a_list_is_a_vec_of_at_most_its_capacity() {
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let (mut list, mut model): (FixedList<u64, 12>, Vec<u64>) = (FixedList::new(), Vec::new());
    for step in 0..5_000 {
        match random.below(8) {
            0..=4 if model.len() < 12 => {
                let item = random.draw();
                list.push(item);
                model.push(item);
            }
            5 | 6 => assert_eq!(list.pop(), model.pop(), "step {step}"),
            7 if random.below(8) == 0 => {
                list.clear();
                model.clear();
            }
            _ => {}
        }
        assert_eq!((&*list, list.is_empty()), (&model[..], model.is_empty()), "step {step}");
    }
}
