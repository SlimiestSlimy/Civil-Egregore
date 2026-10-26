//! Simulated time: the simulation second ([`TICKS_PER_SIMULATION_SECOND`]),
//! what every span of simulated time is counted in, so the tick rate
//! changes here alone; and a span of it ([`SimulationSeconds`]), exact,
//! never a float.

use std::fmt;
use utilities::chance::{Chance, PARTS};

/// Ticks in a simulation second.
pub const TICKS_PER_SIMULATION_SECOND: u64 = 256;

/// The most decimal places a span is written with.
const MOST_DECIMAL_PLACES: u32 = 18;

/// A span of simulated time, more than none: `whole_number` over 10 to
/// the `decimal_places` seconds, as a decimal says it -- no trailing 0
/// after the point, so a span is one value each way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationSeconds {
    /// Its digits, the point taken out.
    whole_number: u64,
    /// How many of them are after the point.
    decimal_places: u32,
}

impl SimulationSeconds {
    /// The span `text` writes in seconds -- digits, then a point and
    /// digits if it wants them, more than 0 -- or `None`.
    pub fn read(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let fraction = fraction.trim_end_matches('0');
        let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty() || !all_digits(whole) || !all_digits(fraction) || fraction.len() > MOST_DECIMAL_PLACES as usize {
            return None;
        }
        let whole_number = format!("{whole}{fraction}").parse().ok().filter(|&number| number > 0)?;
        Some(Self { whole_number, decimal_places: fraction.len() as u32 })
    }

    /// The chance a thing happens in a tick, happening once in this span
    /// on average: always, if the span is a tick or shorter.
    pub fn chance_per_tick(self) -> Chance {
        // 1 over the span in ticks: 10^places over whole_number * ticks, in parts.
        let parts_over_one = u128::from(PARTS) * 10u128.pow(self.decimal_places);
        let ticks_times_power = u128::from(self.whole_number) * u128::from(TICKS_PER_SIMULATION_SECOND);
        let parts = (parts_over_one + ticks_times_power / 2) / ticks_times_power;
        Chance::of_parts(u64::try_from(parts).unwrap_or(PARTS))
    }
}

impl fmt::Display for SimulationSeconds {
    /// The span as [`SimulationSeconds::read`] reads it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = format!("{:0>width$}", self.whole_number, width = self.decimal_places as usize + 1);
        let (whole, fraction) = digits.split_at(digits.len() - self.decimal_places as usize);
        match fraction {
            "" => write!(formatter, "{whole}"),
            _ => write!(formatter, "{whole}.{fraction}"),
        }
    }
}
