//! Chances and the fixed-point logarithm under them: whole numbers
//! that come to what the floats they replace came to.
//!
//! `cargo test`

use utilities::chance::{Chance, PARTS};
use utilities::fixed_point::{log2, log2_by_squaring, LOG2_FRACTION_BITS};

/// How far the logarithm may be off the one found by squaring, in its
/// last bit's worth.
const LAST_BITS: u64 = 4;
use utilities::rng::Rng;

/// The logarithm is exact for every power of two, and for numbers
/// drawn of every size is the float's to within a part in 2^40, the
/// one found by squaring's to within a few of its last bits, and never
/// more than that under the next number's.
#[test]
fn the_logarithm_is_the_floats_and_the_squarings() {
    for power in 0..u64::BITS {
        assert_eq!(log2(1 << power), u64::from(power) << LOG2_FRACTION_BITS);
    }
    let mut random = Rng::new(utilities::seed::counted());
    let one = (1u64 << LOG2_FRACTION_BITS) as f64;
    for _ in 0..400_000 {
        let value = (random.draw() >> random.below(64)).max(1);
        let (fixed, float) = (log2(value) as f64 / one, (value as f64).log2());
        assert!((fixed - float).abs() < 1.0 / (1u64 << 40) as f64, "log2({value}): {fixed} against {float}");
        assert!(log2(value).abs_diff(log2_by_squaring(value, LOG2_FRACTION_BITS)) <= LAST_BITS, "log2({value}): {} against {} by squaring", log2(value), log2_by_squaring(value, LOG2_FRACTION_BITS));
        if value < u64::MAX {
            assert!(log2(value) <= log2(value + 1) + LAST_BITS, "{value}");
        }
    }
    // Every row of the tables, at its ends and beside them.
    for row in 0..=128u64 {
        for value in [(128 + row) << 40, ((128 + row) << 40) - 1, ((128 + row) << 40) + 1, (128 + row) << 55, ((128 + row) << 55) - 1] {
            assert!(log2(value).abs_diff(log2_by_squaring(value, LOG2_FRACTION_BITS)) <= LAST_BITS, "log2({value})");
        }
    }
    assert_eq!(log2(u64::MAX) >> LOG2_FRACTION_BITS, 63);
}

/// A chance is the parts it is made of: once in so many times to the
/// nearest part, two together their sum and no more than certain.
#[test]
fn a_chance_is_its_parts() {
    assert_eq!((Chance::NEVER.parts(), Chance::ALWAYS.parts(), Chance::HALF.parts()), (0, PARTS, PARTS / 2));
    assert_eq!(Chance::one_in(1), Chance::ALWAYS);
    assert_eq!(Chance::one_in(2), Chance::HALF);
    assert_eq!(Chance::one_in(4).plus(Chance::one_in(4)), Chance::HALF);
    assert_eq!(Chance::HALF.plus(Chance::HALF).plus(Chance::HALF), Chance::ALWAYS);
    assert_eq!(Chance::of_parts(PARTS + 5), Chance::ALWAYS);
    assert!(Chance::NEVER.is_never() && Chance::ALWAYS.is_always() && !Chance::HALF.is_never() && !Chance::HALF.is_always());
    for times in [3, 10, 1_000, 100_000, 200_000, 10_000_000] {
        let chance = Chance::one_in(times);
        assert!((chance.fraction() * times as f64 - 1.0).abs() < times as f64 / PARTS as f64, "once in {times}");
    }
}

/// The gap a chance draws is the geometric law's: for chances from a
/// half to the smallest there is, the gap from a draw is the one the
/// float formula gives -- to within a part in a million, where the two
/// round apart -- and over many draws their mean is what the law says,
/// within five standard deviations.
#[test]
fn the_gaps_are_the_geometric_laws() {
    let mut random = Rng::new(utilities::seed::counted());
    for chance in [Chance::HALF, Chance::one_in(3), Chance::one_in(100), Chance::one_in(100_000), Chance::one_in(200_000).plus(Chance::one_in(100_000)), Chance::of_parts(PARTS - 1), Chance::of_parts(7), Chance::of_parts(1)] {
        let likely = chance.fraction();
        let (draws, mut sum) = (200_000, 0.0);
        for _ in 0..draws {
            let draw = random.draw();
            let gap = chance.passed_over(draw);
            let uniform = ((1u64 << 53) - (draw >> 11)) as f64 / (1u64 << 53) as f64;
            let float = (uniform.ln() / (-likely).ln_1p()).floor();
            assert!((gap as f64 - float).abs() <= 1.0 + float * 2e-5, "{chance:?}, draw {draw}: {gap} against {float}");
            sum += gap as f64;
        }
        let (mean, deviation) = ((1.0 - likely) / likely, (1.0 - likely).sqrt() / likely);
        assert!((sum / draws as f64 - mean).abs() <= 5.0 * deviation / (draws as f64).sqrt() + mean * 2e-5, "{chance:?}: mean {} against {mean}", sum / draws as f64);
    }
    assert_eq!(Chance::one_in(1_000).passed_over(0), 0, "the draw that is 1: no gap");
}

/// A draw against a chance comes true as often as it says, within five
/// standard deviations; never for never, always for always; and one
/// thing among two as often as its share of both.
#[test]
fn a_draw_comes_true_as_often_as_the_chance_says() {
    let mut random = Rng::new(utilities::seed::counted());
    let draws = 400_000u64;
    let within = |count: u64, likely: f64| (count as f64 - draws as f64 * likely).abs() <= 5.0 * (draws as f64 * likely * (1.0 - likely)).sqrt();
    for chance in [Chance::NEVER, Chance::one_in(1_000), Chance::one_in(3), Chance::HALF, Chance::ALWAYS] {
        let count = (0..draws).filter(|_| random.chance(chance)).count() as u64;
        assert!(within(count, chance.fraction()), "{chance:?}: {count} of {draws}");
    }
    let (part, other) = (Chance::one_in(100_000), Chance::one_in(200_000));
    let count = (0..draws).filter(|_| random.chance_among(part, part.plus(other))).count() as u64;
    assert!(within(count, 2.0 / 3.0), "two parts in three: {count} of {draws}");
}
