//! Fast tests: a small corpus from a seed drawn fresh each run --
//! every shape, sparse shape, plan and line set, at its `tested` count;
//! and each family, and the saved bitmaps, turned every way round.
//!
//! `cargo test --test fast`

mod tests;

use tests::saved::every_saved;
use tessera::corpus::{corpus_families, HowMany, LINE_SETS, CITY_PLANS, GROWN_SHAPES, SPARSE_SHAPES};
use tests::{check_bitmap, check_turned_bits};

/// How far, in percent, a family's bits may move turned a quarter, a
/// half or three quarters, at the tested counts -- as few as 6 bitmaps a
/// family, so a total moves more than at the timed counts: wide enough
/// that no seed's families have come near it, and still far under the
/// bias the saved horizontal-streaks bitmap once had for one
/// orientation.
const MOST_TURNED_DRIFT_PERCENT: f64 = 5.0;

/// Every shape and sparse shape, at its `tested` count, passes every check in
/// `tests::check`: covered, capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
fn every_shape_round_trips() {
    let seed = tests::fresh_test_seed();
    for grown_shape in GROWN_SHAPES.iter().chain(&SPARSE_SHAPES) {
        for (case_index, bitmap) in grown_shape.tested_bitmaps(seed).enumerate() {
            check_bitmap(&bitmap, &format!("{}, case {case_index}", grown_shape.display_name));
        }
    }
}

/// Every city plan, at its `tested` count, passes every check in
/// `tests::check`: covered, capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
fn every_plan_round_trips() {
    let seed = tests::fresh_test_seed();
    for city_plan in &CITY_PLANS {
        for (case_index, bitmap) in city_plan.tested_bitmaps(seed).enumerate() {
            check_bitmap(&bitmap, &format!("{}, case {case_index}", city_plan.display_name));
        }
    }
}

/// Every line set, at its `tested` count, passes every check in
/// `tests::check`: covered, capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
fn every_line_set_round_trips() {
    let seed = tests::fresh_test_seed();
    for line_set in &LINE_SETS {
        for (case_index, bitmap) in line_set.tested_bitmaps(seed).enumerate() {
            check_bitmap(&bitmap, &format!("{}, case {case_index}", line_set.display_name));
        }
    }
}

/// Every family, at its `tested` count, and the saved adversarial
/// bitmaps take about as many bits turned any way round: each turn's
/// total within [`MOST_TURNED_DRIFT_PERCENT`] of the total as drawn.
#[test]
fn turned_bitmaps_take_about_as_many_bits() {
    let seed = tests::fresh_test_seed();
    let mut family_sets = corpus_families(seed, HowMany::TestedCount);
    family_sets.push(("the saved adversarial bitmaps".to_string(), every_saved().into_iter().map(|(_, bitmap)| bitmap).collect()));
    check_turned_bits(family_sets, MOST_TURNED_DRIFT_PERCENT);
}
