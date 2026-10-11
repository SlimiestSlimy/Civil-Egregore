//! Simulation time: how long a thing takes in a world is said in
//! seconds of the simulation, and turned to ticks here alone -- so
//! nothing that says a time says the tick rate, and the rate is
//! changed in one place (`docs/simulation.md`, "Simulation time").

use utilities::chance::Chance;

/// Ticks in a second of the simulation: the tick rate, written here
/// and nowhere else.
pub const TICKS_A_SIMULATION_SECOND: u64 = 256;

/// Milliseconds in a second.
const MILLISECONDS_A_SECOND: u64 = 1000;

/// A length of simulation time, kept as the ticks it takes at
/// [`TICKS_A_SIMULATION_SECOND`]: the nearest whole number of them,
/// and one at least -- nothing takes less than a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulationTime {
    /// The ticks it takes.
    ticks: u64,
}

impl SimulationTime {
    /// So many seconds of the simulation.
    pub const fn seconds(seconds: u64) -> Self {
        Self::milliseconds(seconds * MILLISECONDS_A_SECOND)
    }

    /// So many milliseconds of the simulation, to the nearest tick.
    pub const fn milliseconds(milliseconds: u64) -> Self {
        let ticks = (milliseconds * TICKS_A_SIMULATION_SECOND + MILLISECONDS_A_SECOND / 2) / MILLISECONDS_A_SECOND;
        Self { ticks: if ticks == 0 { 1 } else { ticks } }
    }

    /// The ticks it takes.
    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    /// The chance, each tick, of a thing that takes this long on
    /// average to happen -- its mean simulation time: once in its
    /// ticks, so the mean is the same whatever the tick rate.
    pub const fn chance_a_tick(self) -> Chance {
        Chance::one_in(self.ticks)
    }
}
