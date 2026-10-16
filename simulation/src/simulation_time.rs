//! Simulation time: how long a thing takes in a world is said in
//! seconds of the simulation, and turned to ticks here alone -- so
//! nothing that says a time says the tick rate, and the rate is
//! changed in one place (`docs/simulation.md`, "Simulation time").

use utilities::chance::Chance;
use utilities::rng::RandomSource;

/// Ticks in a second of the simulation: the tick rate, written here
/// and nowhere else.
pub const TICKS_A_SIMULATION_SECOND: u64 = 256;

/// Milliseconds in a second.
const MILLISECONDS_A_SECOND: u64 = 1000;

/// A length of simulation time, kept as the ticks it takes at
/// [`TICKS_A_SIMULATION_SECOND`]: the nearest whole number of them,
/// and one at least when it is said in seconds -- nothing takes less
/// than a tick; one drawn or measured may be none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulationTime {
    /// The ticks it takes.
    ticks: u64,
}

impl SimulationTime {
    /// So many seconds of the simulation.
    pub const fn from_seconds(simulation_seconds: u64) -> Self {
        Self::from_milliseconds(simulation_seconds * MILLISECONDS_A_SECOND)
    }

    /// So many milliseconds of the simulation, to the nearest tick.
    pub const fn from_milliseconds(simulation_milliseconds: u64) -> Self {
        let ticks = (simulation_milliseconds * TICKS_A_SIMULATION_SECOND + MILLISECONDS_A_SECOND / 2) / MILLISECONDS_A_SECOND;
        Self { ticks: if ticks == 0 { 1 } else { ticks } }
    }

    /// The ticks it takes.
    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    /// The tick this long after the tick `start_tick`.
    pub const fn tick_after(self, start_tick: u64) -> u64 {
        start_tick + self.ticks
    }

    /// The time from the tick `start_tick` to the tick `end_tick`.
    pub const fn time_between_ticks(start_tick: u64, end_tick: u64) -> Self {
        Self { ticks: end_tick - start_tick }
    }

    /// A time shorter than this, drawn from `random_stream`: any tick of it
    /// as likely as another.
    pub fn part_drawn(self, random_stream: &mut RandomSource) -> Self {
        Self { ticks: random_stream.draw_below(self.ticks) }
    }

    /// The chance, each tick, of a thing that takes this long on
    /// average to happen -- its mean simulation time: once in its
    /// ticks, so the mean is the same whatever the tick rate.
    pub const fn chance_a_tick(self) -> Chance {
        Chance::one_in(self.ticks)
    }

    /// The chance, each time something happens that happens once in
    /// `mean_time_between` on average -- a meal, a sample -- of a thing that
    /// takes this long on average: `mean_time_between` of this, the tick rate
    /// in neither; always, if this is no longer than `mean_time_between`.
    pub const fn chance_each(self, mean_time_between: Self) -> Chance {
        Chance::of_parts((mean_time_between.ticks as u128 * utilities::chance::PARTS as u128 / self.ticks as u128) as u64)
    }
}
