//! The climate a world is generated with: for each tile of the
//! climate plane its humidity, wind and temperature, from smooth
//! noise of the world's seed and the ground under it -- the same
//! whichever superchunk is made first, so no seam where two meet
//! (`docs/worldgen.md`, "Climate").

use crate::{Height, Noise};
use type_registry::{Climate, Wind, HOTTEST, MOST_HUMID, STRONGEST_WIND, WAYS_OF_WIND};

/// Mixed with a world's seed for its climate's noise: it lies apart
/// from the ground and from what grows.
pub const CLIMATE_SALT: u64 = 0x636C_696D_6174_6521;
/// Cells from a point to the next of the noise the temperature and the
/// wind lie by, as a power of two: regions many superchunks across.
const BROAD: u32 = 12;
/// Cells from a point to the next of the finer noise, as a power of two.
const FINER: u32 = 9;
/// Steps of height above the ocean that cool a tile by one.
const STEPS_A_DEGREE_COOLER: u32 = 2;
/// Steps of height above the ocean that dry a tile by one.
const STEPS_A_LEVEL_DRIER: u32 = 32;
/// How much more humid a tile of water is than the noise has it.
const MORE_HUMID_OVER_WATER: u32 = 3;

/// The noises a world's climate lies by, kept tile after tile.
pub struct ClimateNoises {
    /// The temperature's: its regions, and the finer.
    temperature: [Noise; 2],
    /// The humidity's: its regions, and the finer.
    humidity: [Noise; 2],
    /// Which way the wind blows.
    way: Noise,
    /// How strong it blows.
    strength: Noise,
}

impl ClimateNoises {
    /// Those of the world of `seed`.
    pub const fn new(seed: u64) -> Self {
        let seed = seed ^ CLIMATE_SALT;
        Self {
            temperature: [Noise::new(seed, 0, BROAD), Noise::new(seed, 1, FINER)],
            humidity: [Noise::new(seed, 2, BROAD - 1), Noise::new(seed, 3, FINER - 1)],
            way: Noise::new(seed, 4, BROAD),
            strength: Noise::new(seed, 5, BROAD - 1),
        }
    }

    /// The climate of the tile whose middle cell is `(x, y)`, the
    /// ground there `above_ocean` steps over the ocean's level, or
    /// `under_water`: the regions' noise three parts and the finer
    /// one, the higher the ground the cooler and the drier, water
    /// milder -- half way to neither hot nor cold -- and more humid.
    pub fn at(&mut self, x: u32, y: u32, above_ocean: Height, under_water: bool) -> Climate {
        // Sixteen bits, the regions' three parts and the finer one.
        let blended = |noises: &mut [Noise; 2]| (3 * noises[0].at(x, y) + noises[1].at(x, y)) / 4;
        let above = u32::from(above_ocean);
        let temperature = (blended(&mut self.temperature) >> 8) as u32;
        let humidity = (blended(&mut self.humidity) >> 12) as u32;
        let (temperature, humidity) = if under_water {
            ((temperature + HOTTEST / 2) / 2, (humidity + MORE_HUMID_OVER_WATER).min(MOST_HUMID))
        } else {
            (temperature.saturating_sub(above / STEPS_A_DEGREE_COOLER), humidity.saturating_sub(above / STEPS_A_LEVEL_DRIER))
        };
        // A region a way, the ways turning one into the next; the strength of 0, calm, to the strongest.
        let way = ((self.way.at(x, y) * WAYS_OF_WIND.len() as u64) >> 16) as usize;
        let strength = ((self.strength.at(x, y) * u64::from(STRONGEST_WIND + 1)) >> 16) as u32;
        Climate { humidity, wind: Wind::blowing(way, strength), temperature }
    }
}
