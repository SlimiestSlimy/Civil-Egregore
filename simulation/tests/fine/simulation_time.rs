//! Simulation time: a length of it is the ticks it takes at the tick
//! rate, to the nearest and one at least; and a thing with a mean
//! time happens, tick by tick, that long apart on average.
//!
//! `cargo test --test fine simulation_time`

use simulation::simulation_time::{SimulationTime, TICKS_A_SIMULATION_SECOND};
use utilities::rng::Rng;

/// Seconds are the tick rate's ticks each, milliseconds their share
/// to the nearest tick, and nothing takes less than one tick.
#[test]
fn a_time_is_its_ticks_at_the_tick_rate() {
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..1000 {
        let seconds = random.below(1 << 20);
        assert_eq!(SimulationTime::seconds(seconds).ticks(), (seconds * TICKS_A_SIMULATION_SECOND).max(1));
        assert_eq!(SimulationTime::seconds(seconds), SimulationTime::milliseconds(seconds * 1000));
        let milliseconds = random.below(1 << 30);
        let (ticks, exact) = (SimulationTime::milliseconds(milliseconds).ticks(), milliseconds * TICKS_A_SIMULATION_SECOND);
        // Within half a tick of the time said, in thousandths of a tick -- or the one tick the shortest takes.
        assert!((ticks * 1000).abs_diff(exact) <= 500 || ticks == 1, "{milliseconds} ms: {ticks} ticks");
    }
    assert_eq!((SimulationTime::milliseconds(0).ticks(), SimulationTime::milliseconds(1).ticks()), (1, 1));
}

/// The chance of a thing each time another happens is the one time
/// in the other, whatever the tick rate: drawn at each of things a
/// mean time apart, it happens its own mean time apart, within a
/// twentieth, over twenty thousand of them -- and always, if it
/// takes no longer than what it is drawn at.
#[test]
fn a_chance_each_time_another_happens_keeps_its_own_mean_time() {
    let mut random = Rng::new(utilities::seed::counted());
    let every = SimulationTime::milliseconds(50 + random.below(500));
    let mean = SimulationTime::milliseconds(every.ticks() * 1000 / TICKS_A_SIMULATION_SECOND * (2 + random.below(20)));
    let (at_each, a_tick) = (mean.chance_each(every), every.chance_a_tick());
    let (mut happened, mut ticks) = (0u64, 0u64);
    while happened < 20_000 {
        ticks += 1;
        happened += u64::from(random.chance(a_tick) && random.chance(at_each));
    }
    let apart = ticks as f64 / happened as f64;
    assert!((apart / mean.ticks() as f64 - 1.0).abs() < 0.05, "{apart} ticks apart, a mean of {}, drawn every {}", mean.ticks(), every.ticks());
    assert!(every.chance_each(mean).is_always() && every.chance_each(every).is_always());
}

/// A thing drawn each tick with the chance its mean time gives
/// happens that long apart, on average: within a twentieth, over
/// twenty thousand of them.
#[test]
fn a_mean_time_is_the_mean_of_what_its_chance_draws() {
    let mut random = Rng::new(utilities::seed::counted());
    let mean = SimulationTime::milliseconds(100 + random.below(2000));
    let (chance, mut happened, mut ticks) = (mean.chance_a_tick(), 0u64, 0u64);
    while happened < 20_000 {
        ticks += 1;
        happened += u64::from(random.chance(chance));
    }
    let apart = ticks as f64 / happened as f64;
    assert!((apart / mean.ticks() as f64 - 1.0).abs() < 0.05, "{apart} ticks apart, a mean of {}", mean.ticks());
}
