//! What the climate plane's sixteen bits a tile hold ([`Climate`]):
//! humidity, wind and temperature kept together, and the wind's
//! thirteen states ([`Wind`]) (`docs/type_registry.md`, "The climate").

/// The level of cloud above which a tile is raining.
pub const RAINS_ABOVE: u32 = 12;
/// The most humid a tile is: four bits.
pub const MOST_HUMID: u32 = 15;
/// The hottest a tile is: eight bits.
pub const HOTTEST: u32 = 255;
/// The strongest a wind blows.
pub const STRONGEST_WIND: u32 = 3;
/// The ways a wind blows towards, in the order its states name them:
/// north, west, south, east -- each the step to the next tile, across
/// and down.
pub const WAYS_OF_WIND: [(i32, i32); 4] = [(0, -1), (-1, 0), (0, 1), (1, 0)];

/// The wind over a tile: calm, or blowing towards one of
/// [`WAYS_OF_WIND`] at a strength of 1 to [`STRONGEST_WIND`]. Four
/// bits: 0 calm, 1 to 4 the four ways at strength 1, 5 to 8 at 2, 9 to
/// 12 at 3; 13 to 15 are no wind's, and read as calm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wind(u8);

impl Wind {
    /// No wind.
    pub const CALM: Self = Self(0);

    /// The wind blowing towards `WAYS_OF_WIND[way]` at `strength`: calm
    /// at none.
    pub const fn blowing(way: usize, strength: u32) -> Self {
        assert!(way < WAYS_OF_WIND.len() && strength <= STRONGEST_WIND, "a wind blows one of four ways at a strength up to three");
        if strength == 0 { Self::CALM } else { Self(((strength - 1) * WAYS_OF_WIND.len() as u32 + way as u32 + 1) as u8) }
    }

    /// The wind the four bits `state` say.
    pub const fn of_state(state: u32) -> Self {
        if state > STRONGEST_WIND * WAYS_OF_WIND.len() as u32 { Self::CALM } else { Self(state as u8) }
    }

    /// Its four bits.
    pub const fn state(self) -> u32 {
        self.0 as u32
    }

    /// How strong it blows: 0 calm, to [`STRONGEST_WIND`].
    pub const fn strength(self) -> u32 {
        (self.0 as u32 + WAYS_OF_WIND.len() as u32 - 1) / WAYS_OF_WIND.len() as u32
    }

    /// Which of [`WAYS_OF_WIND`] it blows towards: none when calm.
    pub const fn way(self) -> Option<usize> {
        if self.0 == 0 { None } else { Some((self.0 as usize - 1) % WAYS_OF_WIND.len()) }
    }

    /// The step to the tile it blows towards, in tiles across and
    /// down: none when calm.
    pub const fn towards(self) -> Option<(i32, i32)> {
        match self.way() {
            Some(way) => Some(WAYS_OF_WIND[way]),
            None => None,
        }
    }
}

/// A tile's climate: what the climate plane's number is made of, and
/// read back as. Its lowest four bits the humidity, the next four the
/// wind, the highest eight the temperature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Climate {
    /// How much water its air holds: 0 to [`MOST_HUMID`].
    pub humidity: u32,
    /// The wind over it.
    pub wind: Wind,
    /// How hot it is: 0 to [`HOTTEST`].
    pub temperature: u32,
}

impl Climate {
    /// The climate the plane's number `number` says.
    pub const fn of_number(number: u32) -> Self {
        Self { humidity: number & MOST_HUMID, wind: Wind::of_state(number >> 4 & 0xF), temperature: number >> 8 & HOTTEST }
    }

    /// The number the plane holds for it.
    pub const fn number(self) -> u32 {
        assert!(self.humidity <= MOST_HUMID && self.temperature <= HOTTEST, "a climate within its bits");
        self.humidity | self.wind.state() << 4 | self.temperature << 8
    }
}
