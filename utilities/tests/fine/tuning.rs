//! The numbers tuned, against the two files written by hand.

use utilities::tuning::{tuned_number, unless_set, NAMES};

/// Every number has its slider's line and its default, the default a
/// number: neither file has fallen behind the names.
#[test]
fn every_number_has_a_slider_and_a_default() {
    for (number_index, setting_name) in NAMES.into_iter().enumerate() {
        let (least_value, most_value) = tuned_number(number_index).slider_range;
        assert!(tuned_number(number_index).setting_name == setting_name && least_value < most_value && !tuned_number(number_index).what_it_does.is_empty(), "{setting_name}");
        assert!(unless_set(number_index).is_finite(), "{setting_name}");
    }
}
