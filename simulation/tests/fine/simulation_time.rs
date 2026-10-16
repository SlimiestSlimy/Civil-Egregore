//! Simulation time: a length of it is the ticks it takes at the tick
//! rate, to the nearest and one at least; and a thing with a mean
//! time happens, tick by tick, that long apart on average.
//!
//! `cargo test --test fine simulation_time`

use simulation::simulation_time::{SimulationTime, TICKS_A_SIMULATION_SECOND};
use utilities::rng::RandomSource;

/// Seconds are the tick rate's ticks each, milliseconds their share
/// to the nearest tick, and nothing takes less than one tick.
#[test]
fn a_time_is_its_ticks_at_the_tick_rate() {
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    for _ in 0..1000 {
        let drawn_seconds = seeded_rng.draw_below(1 << 20);
        assert_eq!(SimulationTime::from_seconds(drawn_seconds).ticks(), (drawn_seconds * TICKS_A_SIMULATION_SECOND).max(1));
        assert_eq!(SimulationTime::from_seconds(drawn_seconds), SimulationTime::from_milliseconds(drawn_seconds * 1000));
        let drawn_milliseconds = seeded_rng.draw_below(1 << 30);
        let (ticks, exact_thousandths) = (SimulationTime::from_milliseconds(drawn_milliseconds).ticks(), drawn_milliseconds * TICKS_A_SIMULATION_SECOND);
        // Within half a tick of the time said, in thousandths of a tick -- or the one tick the shortest takes.
        assert!((ticks * 1000).abs_diff(exact_thousandths) <= 500 || ticks == 1, "{drawn_milliseconds} ms: {ticks} ticks");
    }
    assert_eq!((SimulationTime::from_milliseconds(0).ticks(), SimulationTime::from_milliseconds(1).ticks()), (1, 1));
}

/// The chance of a thing each time another happens is the one time
/// in the other, whatever the tick rate: drawn at each of things a
/// mean time apart, it happens its own mean time apart, within a
/// twentieth, over twenty thousand of them -- and always, if it
/// takes no longer than what it is drawn at.
#[test]
fn a_chance_each_time_another_happens_keeps_its_own_mean_time() {
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    let drawn_at_time = SimulationTime::from_milliseconds(50 + seeded_rng.draw_below(500));
    let mean_time = SimulationTime::from_milliseconds(drawn_at_time.ticks() * 1000 / TICKS_A_SIMULATION_SECOND * (2 + seeded_rng.draw_below(20)));
    let (at_each, a_tick) = (mean_time.chance_each(drawn_at_time), drawn_at_time.chance_a_tick());
    let (mut happened_count, mut ticks) = (0u64, 0u64);
    while happened_count < 20_000 {
        ticks += 1;
        happened_count += u64::from(seeded_rng.chance(a_tick) && seeded_rng.chance(at_each));
    }
    let ticks_apart = ticks as f64 / happened_count as f64;
    assert!((ticks_apart / mean_time.ticks() as f64 - 1.0).abs() < 0.05, "{ticks_apart} ticks apart, a mean of {}, drawn every {}", mean_time.ticks(), drawn_at_time.ticks());
    assert!(drawn_at_time.chance_each(mean_time).is_always() && drawn_at_time.chance_each(drawn_at_time).is_always());
}

/// A thing drawn each tick with the chance its mean time gives
/// happens that long apart, on average: within a twentieth, over
/// twenty thousand of them.
#[test]
fn a_mean_time_is_the_mean_of_what_its_chance_draws() {
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    let mean_time = SimulationTime::from_milliseconds(100 + seeded_rng.draw_below(2000));
    let (chance, mut happened_count, mut ticks) = (mean_time.chance_a_tick(), 0u64, 0u64);
    while happened_count < 20_000 {
        ticks += 1;
        happened_count += u64::from(seeded_rng.chance(chance));
    }
    let ticks_apart = ticks as f64 / happened_count as f64;
    assert!((ticks_apart / mean_time.ticks() as f64 - 1.0).abs() < 0.05, "{ticks_apart} ticks apart, a mean of {}", mean_time.ticks());
}
