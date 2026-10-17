//! Chances and the fixed-point logarithm under them: whole numbers
//! that come to what the floats they replace came to.
//!
//! `cargo test`

use utilities::chance::{Chance, PARTS};
use utilities::fixed_point::{log2_by_table, log2_by_squaring, LOG2_FRACTION_BITS};

/// How far the logarithm may be off the one found by squaring, in its
/// last bit's worth.
const LAST_BITS: u64 = 4;
use utilities::rng::RandomSource;

/// The logarithm is exact for every power of two, and for numbers
/// drawn of every size is the float's to within a part in 2^40, the
/// one found by squaring's to within a few of its last bits, and never
/// more than that under the next number's.
#[test]
fn the_logarithm_is_the_floats_and_the_squarings() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    for power_of_two in 0..u64::BITS {
        assert_eq!(log2_by_table(1 << power_of_two), u64::from(power_of_two) << LOG2_FRACTION_BITS);
    }
    let mut random_numbers = RandomSource::new(seed);
    let fixed_one = (1u64 << LOG2_FRACTION_BITS) as f64;
    for _ in 0..400_000 {
        let value = (random_numbers.next_draw() >> random_numbers.draw_below(64)).max(1);
        let (fixed_log, float_log) = (log2_by_table(value) as f64 / fixed_one, (value as f64).log2());
        assert!((fixed_log - float_log).abs() < 1.0 / (1u64 << 40) as f64, "log2({value}): {fixed_log} against {float_log}");
        assert!(log2_by_table(value).abs_diff(log2_by_squaring(value, LOG2_FRACTION_BITS)) <= LAST_BITS, "log2({value}): {} against {} by squaring", log2_by_table(value), log2_by_squaring(value, LOG2_FRACTION_BITS));
        if value < u64::MAX {
            assert!(log2_by_table(value) <= log2_by_table(value + 1) + LAST_BITS, "{value}");
        }
    }
    // Every row of the tables, at its ends and beside them.
    for table_row in 0..=128u64 {
        for value in [(128 + table_row) << 40, ((128 + table_row) << 40) - 1, ((128 + table_row) << 40) + 1, (128 + table_row) << 55, ((128 + table_row) << 55) - 1] {
            assert!(log2_by_table(value).abs_diff(log2_by_squaring(value, LOG2_FRACTION_BITS)) <= LAST_BITS, "log2({value})");
        }
    }
    assert_eq!(log2_by_table(u64::MAX) >> LOG2_FRACTION_BITS, 63);
}

/// A chance is the parts it is made of: once in so many times to the
/// nearest part, two together their sum and no more than certain.
#[test]
fn a_chance_is_its_parts() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    assert_eq!((Chance::NEVER_HAPPENS.parts(), Chance::ALWAYS_HAPPENS.parts(), Chance::HALF_THE_TIME.parts()), (0, PARTS, PARTS / 2));
    assert_eq!(Chance::one_in(1), Chance::ALWAYS_HAPPENS);
    assert_eq!(Chance::one_in(2), Chance::HALF_THE_TIME);
    assert_eq!(Chance::one_in(4).plus_disjoint(Chance::one_in(4)), Chance::HALF_THE_TIME);
    assert_eq!(Chance::HALF_THE_TIME.plus_disjoint(Chance::HALF_THE_TIME).plus_disjoint(Chance::HALF_THE_TIME), Chance::ALWAYS_HAPPENS);
    assert_eq!(Chance::of_parts(PARTS + 5), Chance::ALWAYS_HAPPENS);
    assert!(Chance::NEVER_HAPPENS.is_never() && Chance::ALWAYS_HAPPENS.is_always() && !Chance::HALF_THE_TIME.is_never() && !Chance::HALF_THE_TIME.is_always());
    let mut random_numbers = RandomSource::new(seed);
    // Once in a few times and once in very many, and numbers of times drawn of every size between.
    let drawn_times: Vec<u64> = (0..1_000).map(|_| 1 + (random_numbers.next_draw() >> random_numbers.draw_between(40, 63))).collect();
    for occasion_count in [3, 10, 1_000, 100_000, 200_000, 10_000_000].into_iter().chain(drawn_times) {
        let chance = Chance::one_in(occasion_count);
        assert!((chance.as_fraction() * occasion_count as f64 - 1.0).abs() < occasion_count as f64 / PARTS as f64, "once in {occasion_count}");
    }
}

/// The gap a chance draws is the geometric law's: for chances from a
/// half to the smallest there is, the gap from a draw is the one the
/// float formula gives -- to within a part in a million, where the two
/// round apart -- and over many draws their mean is what the law says,
/// within five standard deviations.
#[test]
fn the_gaps_are_the_geometric_laws() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    for chance in [Chance::HALF_THE_TIME, Chance::one_in(3), Chance::one_in(100), Chance::one_in(100_000), Chance::one_in(200_000).plus_disjoint(Chance::one_in(100_000)), Chance::of_parts(PARTS - 1), Chance::of_parts(7), Chance::of_parts(1)] {
        let chance_fraction = chance.as_fraction();
        let (draw_count, mut gap_sum) = (200_000, 0.0);
        for _ in 0..draw_count {
            let drawn_bits = random_numbers.next_draw();
            let gap = chance.passed_over(drawn_bits);
            let uniform_draw = ((1u64 << 53) - (drawn_bits >> 11)) as f64 / (1u64 << 53) as f64;
            let float_gap = (uniform_draw.ln() / (-chance_fraction).ln_1p()).floor();
            assert!((gap as f64 - float_gap).abs() <= 1.0 + float_gap * 2e-5, "{chance:?}, draw {drawn_bits}: {gap} against {float_gap}");
            gap_sum += gap as f64;
        }
        let (expected_mean, standard_deviation) = ((1.0 - chance_fraction) / chance_fraction, (1.0 - chance_fraction).sqrt() / chance_fraction);
        assert!((gap_sum / draw_count as f64 - expected_mean).abs() <= 5.0 * standard_deviation / (draw_count as f64).sqrt() + expected_mean * 2e-5, "{chance:?}: mean {} against {expected_mean}", gap_sum / draw_count as f64);
    }
    assert_eq!(Chance::one_in(1_000).passed_over(0), 0, "the draw that is 1: no gap");
}

/// A draw against a chance comes true as often as it says, within five
/// standard deviations; never for never, always for always; and one
/// thing among two as often as its share of both.
#[test]
fn a_draw_comes_true_as_often_as_the_chance_says() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let draw_count = 400_000u64;
    let within_five_deviations = |count: u64, expected_fraction: f64| (count as f64 - draw_count as f64 * expected_fraction).abs() <= 5.0 * (draw_count as f64 * expected_fraction * (1.0 - expected_fraction)).sqrt();
    for chance in [Chance::NEVER_HAPPENS, Chance::one_in(1_000), Chance::one_in(3), Chance::HALF_THE_TIME, Chance::ALWAYS_HAPPENS] {
        let count = (0..draw_count).filter(|_| random_numbers.chance(chance)).count() as u64;
        assert!(within_five_deviations(count, chance.as_fraction()), "{chance:?}: {count} of {draw_count}");
    }
    let (part, other_chance) = (Chance::one_in(100_000), Chance::one_in(200_000));
    let count = (0..draw_count).filter(|_| random_numbers.chance_among(part, part.plus_disjoint(other_chance))).count() as u64;
    assert!(within_five_deviations(count, 2.0 / 3.0), "two parts in three: {count} of {draw_count}");
}
