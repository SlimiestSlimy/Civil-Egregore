//! The numbers tuned, against the two files written by hand.

use utilities::tuning::{tuned, unless_set, NAMES};

/// Every number has its slider's line and its default, the default a
/// number: neither file has fallen behind the names.
#[test]
fn every_number_has_a_slider_and_a_default() {
    for (index, name) in NAMES.into_iter().enumerate() {
        let (least, most) = tuned(index).range;
        assert!(tuned(index).name == name && least < most && !tuned(index).what.is_empty(), "{name}");
        assert!(unless_set(index).is_finite(), "{name}");
    }
}
