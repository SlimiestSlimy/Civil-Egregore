//! The climate a world is generated with: for each tile of the
//! climate plane its humidity, wind and temperature, from smooth
//! noise of the world's seed and the ground under it -- the same
//! whichever superchunk is made first, so no seam where two meet
//! (`docs/worldgen.md`, "Climate").

use crate::worldgen::{Height, SmoothNoise};
use crate::base_types::{Climate, Wind, HOTTEST_TEMPERATURE, MOST_HUMID, PRESSURE_AT_REST, STRONGEST_WIND, TILES_ABOUT};

/// Mixed with a world's seed for its climate's noise: it lies apart
/// from the ground and from what grows.
pub const CLIMATE_SALT: u64 = 0x636C_696D_6174_6521;
/// Cells from a point to the next of the noise the temperature and the
/// wind lie by, as a power of two: regions many superchunks across.
const BROAD_SPACING_POWER: u32 = 12;
/// Cells from a point to the next of the finer noise, as a power of two.
const FINER_SPACING_POWER: u32 = 9;
/// Steps of height above the ocean that cool a tile by one.
const STEPS_A_DEGREE_COOLER: u32 = 2;
/// Steps of height above the ocean that dry a tile by one.
const STEPS_A_LEVEL_DRIER: u32 = 2;
/// How much more humid a tile of water is than the noise has it.
const MORE_HUMID_OVER_WATER: u32 = 48;

/// The noises a world's climate lies by, kept tile after tile.
pub struct ClimateNoises {
    /// The temperature's: its regions, and the finer.
    temperature: [SmoothNoise; 2],
    /// The humidity's: its regions, and the finer.
    humidity: [SmoothNoise; 2],
    /// Which way the wind blows.
    way: SmoothNoise,
    /// How strong it blows.
    strength_noise: SmoothNoise,
}

impl ClimateNoises {
    /// Those of the world of `seed`.
    pub const fn new(seed: u64) -> Self {
        let seed = seed ^ CLIMATE_SALT;
        Self {
            temperature: [SmoothNoise::new(seed, 0, BROAD_SPACING_POWER), SmoothNoise::new(seed, 1, FINER_SPACING_POWER)],
            humidity: [SmoothNoise::new(seed, 2, BROAD_SPACING_POWER - 1), SmoothNoise::new(seed, 3, FINER_SPACING_POWER - 1)],
            way: SmoothNoise::new(seed, 4, BROAD_SPACING_POWER),
            strength_noise: SmoothNoise::new(seed, 5, BROAD_SPACING_POWER - 1),
        }
    }

    /// The climate of the tile whose middle cell is `(x, y)`, the
    /// ground there `above_ocean` steps over the ocean's level, or
    /// `under_water`: the regions' noise three parts and the finer
    /// one, the higher the ground the cooler and the drier, water
    /// milder -- half way to neither hot nor cold -- and more humid.
    pub fn climate_at(&mut self, x: u32, y: u32, above_ocean: Height, under_water: bool) -> Climate {
        // Sixteen bits, the regions' three parts and the finer one.
        let mixed_noise = |noise_pair: &mut [SmoothNoise; 2]| (3 * noise_pair[0].noise_at(x, y) + noise_pair[1].noise_at(x, y)) / 4;
        let steps_above = u32::from(above_ocean);
        let temperature = (mixed_noise(&mut self.temperature) >> 8) as u32;
        let humidity = (mixed_noise(&mut self.humidity) >> 8) as u32;
        let (temperature, humidity) = if under_water {
            ((temperature + HOTTEST_TEMPERATURE / 2) / 2, (humidity + MORE_HUMID_OVER_WATER).min(MOST_HUMID))
        } else {
            (temperature.saturating_sub(steps_above / STEPS_A_DEGREE_COOLER), humidity.saturating_sub(steps_above / STEPS_A_LEVEL_DRIER))
        };
        // A region a way, one of the eight, the ways turning one into the next; the strength of 0, calm, to the strongest. The pressure at rest: the winds make the rest of it.
        let (way_east, way_south) = TILES_ABOUT[((self.way.noise_at(x, y) * TILES_ABOUT.len() as u64) >> 16) as usize];
        let drawn_strength = ((self.strength_noise.noise_at(x, y) * (STRONGEST_WIND as u64 + 1)) >> 16) as i32;
        Climate { humidity, wind: Wind::from_vector(way_east * drawn_strength, way_south * drawn_strength), temperature, pressure: PRESSURE_AT_REST }
    }
}
