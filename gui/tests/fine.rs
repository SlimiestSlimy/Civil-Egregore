//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test -p gui --test fine`

mod tuning {
    //! The numbers tuned, against the two files written by hand.

    use gui::tuning::{tuned, unless_set, NAMES};

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
}
