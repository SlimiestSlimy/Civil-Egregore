//! Complete tests: everything the measurements run on, plus a moderate
//! corpus from a second seed base the measurements never see, plus the
//! checkerboards; and every family turned each way round, taking about
//! as many bits.
//!
//! `cargo test --release --test complete -- --ignored`

mod tests;

use tests::saved::every_saved;
use tessera::corpus::checkerboards::every_checkerboard;
use tessera::corpus::{corpus_families, grown_bitmaps, one_laid_out, HowMany, CITY_PLANS, GROWN_SHAPES};
use tests::{check_bitmap, check_turned_bits};

/// How far from the measured seeds the second corpus starts.
const SECOND_SEED_OFFSET: u64 = 1_000_000;

/// How many of each shape and plan the second corpus takes.
const SECOND_CORPUS_EACH: u64 = 4;

/// Every bitmap of every family, at its `timed` count, passes every check in
/// `tests::check_bitmap`: covered, capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
#[ignore]
fn every_family_round_trips() {
    let seed = tests::fresh_test_seed();
    for (family_name, maps) in corpus_families(seed, HowMany::TimedCount) {
        for (case_index, bitmap) in maps.iter().enumerate() {
            check_bitmap(bitmap, &format!("{family_name}, case {case_index}"));
        }
    }
}

/// A few bitmaps of every shape and plan from seeds the measurements
/// never use, and each passes every check in `tests::check_bitmap`: covered,
/// capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
#[ignore]
fn a_second_seed_base_round_trips() {
    let seed = tests::fresh_test_seed();
    for grown_shape in &GROWN_SHAPES {
        let seed = seed.wrapping_add(SECOND_SEED_OFFSET);
        for (case_index, bitmap) in grown_bitmaps(seed, grown_shape.cell_density, grown_shape.cluster_weight, SECOND_CORPUS_EACH).enumerate() {
            check_bitmap(&bitmap, &format!("{}, second seed base, case {case_index}", grown_shape.display_name));
        }
    }
    for city_plan in &CITY_PLANS {
        let seed = seed.wrapping_add(SECOND_SEED_OFFSET);
        for case_index in 0..SECOND_CORPUS_EACH {
            check_bitmap(&one_laid_out(seed + case_index, city_plan), &format!("{}, second seed base, case {case_index}", city_plan.display_name));
        }
    }
}

/// Every checkerboard of odd-sided squares passes every check in
/// `tests::check_bitmap`: covered, capped, costed as written, and decoded back,
/// whatever follows its stream.
#[test]
#[ignore]
fn every_checkerboard_round_trips() {
    for (square_side, bitmap) in every_checkerboard() {
        check_bitmap(&bitmap, &format!("checkerboard of {square_side}x{square_side} squares"));
    }
}

/// How far, in percent, a family's bits may move turned a quarter, a
/// half or three quarters, at the timed counts: more than twice what any
/// seed's families have moved, and still far under the bias the saved
/// horizontal-streaks bitmap once had for one orientation, when residual
/// floor tiles were counted at a bit a cell.
const MOST_TURNED_DRIFT_PERCENT: f64 = 2.0;

/// Every family, at its `timed` count, and the saved adversarial
/// bitmaps take about as many bits turned any way round: each turn's
/// total within [`MOST_TURNED_DRIFT_PERCENT`] of the total as drawn.
#[test]
#[ignore]
fn turned_bitmaps_take_about_as_many_bits() {
    let seed = tests::fresh_test_seed();
    let mut family_sets = corpus_families(seed, HowMany::TimedCount);
    family_sets.push(("the saved adversarial bitmaps".to_string(), every_saved().into_iter().map(|(_, bitmap)| bitmap).collect()));
    check_turned_bits(family_sets, MOST_TURNED_DRIFT_PERCENT);
}
