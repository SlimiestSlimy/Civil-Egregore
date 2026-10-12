//! What the climate plane's thirty-two bits a tile hold ([`Climate`]):
//! humidity, wind, temperature and pressure, a byte each, kept
//! together; the wind a vector ([`Wind`]); and the small functions
//! the four are connected by (`docs/type_registry.md`, "The climate").

/// The level of cloud above which a tile of the coldest, driest
/// climate is raining: what [`Climate::rains_above`] is worked out
/// from.
pub const RAINS_ABOVE_IN_COLD_DRY_AIR: u32 = 12;
/// The temperature that lets the air hold a level of cloud more
/// before it rains: warm air holds more water.
pub const HEAT_A_LEVEL_MORE_HELD: u32 = 64;
/// The humidity that has the air rain a level of cloud sooner: air
/// already wet has less room.
pub const HUMIDITY_A_LEVEL_SOONER: u32 = 64;
/// The most [`Climate::rains_above`] is: under the deepest cloud it
/// rains in any climate.
pub const RAINS_ABOVE_AT_THE_MOST: u32 = 14;
/// The most humid a tile is: a byte.
pub const MOST_HUMID: u32 = 255;
/// The humidity a level of cloud stands for: the clouds are 0 to 15,
/// the humidity 0 to 255 ([`Climate::humidity_level`]).
pub const HUMIDITY_A_LEVEL_OF_CLOUD: u32 = 16;
/// The hottest a tile is: a byte.
pub const HOTTEST: u32 = 255;
/// The highest a tile's pressure is: a byte.
pub const HIGHEST_PRESSURE: u32 = 255;
/// The pressure a tile is generated at, and all are at with no wind:
/// the middle of what its byte holds.
pub const PRESSURE_AT_REST: u32 = 128;
/// How far apart the pressures of two tiles side by side may be and
/// stay: past it the higher gives the lower a unit
/// ([`pressure_falls_by`]).
pub const PRESSURE_GAP_AT_REST: u32 = 1;
/// The strongest a wind blows, across or down: what a nibble holds
/// either side of none.
pub const STRONGEST_WIND: i32 = 7;
/// The eight tiles about a tile, each the step to it across and
/// down: where a wind blows to and pressure falls to.
pub const TILES_ABOUT: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
/// How much of the humidity or the temperature between two tiles a
/// wind carries from one to the next at once, at the most.
pub const DEGREES_A_WIND_CARRIES: u32 = 4;
/// How much the land holds air back by, as a rise in height a tile
/// to the next: what halves the chance of air going over it
/// ([`passes_a_slope`]). Far more than the land rises from most
/// tiles to the next: a slope leans the air, and seldom stops it.
pub const RISE_THAT_HALVES: u32 = 64;
/// The fall in height, a tile to the next, down which nothing holds
/// air back; level land holds it as a rise of this much does past
/// it, so air goes downhill a little the likelier
/// ([`passes_a_slope`]).
pub const FALL_THAT_FREES: u32 = 8;

/// `from` up to `step` nearer `to`, never past it: how a number a
/// wind carries comes to its neighbour's.
pub const fn nearer(from: u32, to: u32, step: u32) -> u32 {
    if from < to {
        if to - from < step { to } else { from + step }
    } else if from - to < step {
        to
    } else {
        from - step
    }
}

/// The parts of `whole` with which air goes from a tile to one
/// `slope` higher -- lower, under 0 -- by pressure or by wind, the
/// one resistance: all of it down a fall of [`FALL_THAT_FREES`] or
/// more, a little less on the level, and less the steeper the rise,
/// mildly -- half where the rise is [`RISE_THAT_HALVES`] past that
/// fall.
pub const fn passes_a_slope(slope: i32, whole: u32) -> u32 {
    let held_as_by = slope + FALL_THAT_FREES as i32;
    let held_as_by = if held_as_by > 0 { held_as_by as u32 } else { 0 };
    whole * RISE_THAT_HALVES / (RISE_THAT_HALVES + held_as_by)
}

/// How strongly pressure `here` falls to a tile at `there`: by how
/// far that one is under it, and not at all within
/// [`PRESSURE_GAP_AT_REST`] -- so of the tiles about one, the lowest
/// is the likeliest drawn and none higher ever is.
pub const fn pressure_falls_by(here: u32, there: u32) -> u32 {
    if here > there + PRESSURE_GAP_AT_REST { here - there } else { 0 }
}

/// The whole number nearest the square root of `number`.
const fn nearest_root(number: u32) -> u32 {
    // The greatest whole number whose square is no more, a bit at a time from the highest...
    let (mut root, mut bit) = (0u64, 1u64 << 15);
    while bit != 0 {
        if (root | bit) * (root | bit) <= number as u64 {
            root |= bit;
        }
        bit >>= 1;
    }
    // ...and the next, if `number` is past the half way between them.
    if (2 * root + 1) * (2 * root + 1) < 4 * number as u64 { root as u32 + 1 } else { root as u32 }
}

/// The vector `across` and `down` held to a circle of
/// [`STRONGEST_WIND`]: as it is within it, else shortened to it the
/// way it points, to the nearest whole numbers -- so a wind blows no
/// stronger between two of the eight ways than along one.
const fn held_to_the_circle(across: i32, down: i32) -> (i32, i32) {
    let square = across * across + down * down;
    if square <= STRONGEST_WIND * STRONGEST_WIND {
        return (across, down);
    }
    // Its length, in 256ths.
    let length = nearest_root(square as u32 * 256 * 256) as i32;
    ((across * STRONGEST_WIND * 256 + across.signum() * (length / 2)) / length, (down * STRONGEST_WIND * 256 + down.signum() * (length / 2)) / length)
}

/// The wind over a tile: a vector, how strongly it blows across
/// (east the more) and down (south the more), no longer than
/// [`STRONGEST_WIND`] whichever way it points -- a circle, not a
/// square -- so it blows any of eight ways, or between them, at many
/// strengths, and two winds are averaged by their numbers. A byte: a
/// nibble each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wind {
    /// How strongly it blows east: west if under 0.
    across: i8,
    /// How strongly it blows south: north if under 0.
    down: i8,
}

impl Wind {
    /// No wind.
    pub const CALM: Self = Self { across: 0, down: 0 };

    /// The wind blowing `across` east and `down` south, held to
    /// [`STRONGEST_WIND`] whichever way that is
    /// ([`held_to_the_circle`]).
    pub const fn blowing(across: i32, down: i32) -> Self {
        let (across, down) = held_to_the_circle(across, down);
        Self { across: across as i8, down: down as i8 }
    }

    /// The wind the byte `state` says: a nibble each, two's
    /// complement.
    pub const fn of_state(state: u32) -> Self {
        Self { across: (((state & 0xF) as i32) << 28 >> 28) as i8, down: (((state >> 4 & 0xF) as i32) << 28 >> 28) as i8 }
    }

    /// Its byte.
    pub const fn state(self) -> u32 {
        (self.across as u32 & 0xF) | (self.down as u32 & 0xF) << 4
    }

    /// How strongly it blows east and south: under 0, west and north.
    pub const fn vector(self) -> (i32, i32) {
        (self.across as i32, self.down as i32)
    }

    /// How strong it blows: its vector's length, to the nearest whole
    /// number, 0 calm.
    pub const fn strength(self) -> u32 {
        let (across, down) = (self.across as i32, self.down as i32);
        nearest_root((across * across + down * down) as u32)
    }

    /// The step to the tile it blows to this time, across and down,
    /// by two numbers drawn under [`STRONGEST_WIND`] + 1: a step
    /// each way as likely as the wind is strong that way -- so any of
    /// the eight tiles about it, the stronger the likelier. None if
    /// it reaches no tile this time.
    pub const fn step_drawn(self, drawn_across: u32, drawn_down: u32) -> Option<(i32, i32)> {
        let across = if drawn_across < self.across.unsigned_abs() as u32 { self.across.signum() as i32 } else { 0 };
        let down = if drawn_down < self.down.unsigned_abs() as u32 { self.down.signum() as i32 } else { 0 };
        if across == 0 && down == 0 { None } else { Some((across, down)) }
    }

    /// This wind with a push of `across` and `down` added to it: what
    /// air going that way does to the wind over the tile it leaves,
    /// the more of it the stronger.
    pub const fn pushed(self, across: i32, down: i32) -> Self {
        Self::blowing(self.across as i32 + across, self.down as i32 + down)
    }

    /// This wind a step nearer `other`, each of its two ways by one,
    /// held to the circle as any wind: two winds averaged a little at
    /// a time, so winds that differ wear each other down and one wind
    /// over all stays as it is.
    pub const fn a_step_towards(self, other: Self) -> Self {
        Self::blowing((self.across + (other.across - self.across).signum()) as i32, (self.down + (other.down - self.down).signum()) as i32)
    }
}

/// A tile's climate: what the climate plane's number is made of, and
/// read back as. Its lowest byte the humidity, the next the wind,
/// the next the temperature, the highest the pressure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Climate {
    /// How much water its air holds: 0 to [`MOST_HUMID`].
    pub humidity: u32,
    /// The wind over it.
    pub wind: Wind,
    /// How hot it is: 0 to [`HOTTEST`].
    pub temperature: u32,
    /// The pressure of its air: 0 to [`HIGHEST_PRESSURE`],
    /// [`PRESSURE_AT_REST`] where no wind has pushed air in or out.
    pub pressure: u32,
}

impl Climate {
    /// The climate the plane's number `number` says.
    pub const fn of_number(number: u32) -> Self {
        Self { humidity: number & 0xFF, wind: Wind::of_state(number >> 8 & 0xFF), temperature: number >> 16 & 0xFF, pressure: number >> 24 }
    }

    /// The number the plane holds for it.
    pub const fn number(self) -> u32 {
        assert!(self.humidity <= MOST_HUMID && self.temperature <= HOTTEST && self.pressure <= HIGHEST_PRESSURE, "a climate within its bytes");
        self.humidity | self.wind.state() << 8 | self.temperature << 16 | self.pressure << 24
    }

    /// Its humidity as a level of cloud, 0 to 15: what a cloud over
    /// it gathers about.
    pub const fn humidity_level(self) -> u32 {
        self.humidity / HUMIDITY_A_LEVEL_OF_CLOUD
    }

    /// This climate with its humidity up to
    /// [`DEGREES_A_WIND_CARRIES`] nearer that of `upwind`: what a
    /// wind carries of the air it comes from.
    pub const fn with_humidity_carried_from(self, upwind: Self) -> Self {
        Self { humidity: nearer(self.humidity, upwind.humidity, DEGREES_A_WIND_CARRIES), ..self }
    }

    /// This climate with its temperature up to
    /// [`DEGREES_A_WIND_CARRIES`] nearer that of `upwind`.
    pub const fn with_temperature_carried_from(self, upwind: Self) -> Self {
        Self { temperature: nearer(self.temperature, upwind.temperature, DEGREES_A_WIND_CARRIES), ..self }
    }

    /// This climate with its wind a step nearer that of `upwind`
    /// ([`Wind::a_step_towards`]).
    pub const fn with_wind_carried_from(self, upwind: Self) -> Self {
        Self { wind: self.wind.a_step_towards(upwind.wind), ..self }
    }

    /// The level of cloud above which a tile of this climate is
    /// raining: from both its temperature and its humidity -- the
    /// hotter, the more cloud the air holds before it rains; the more
    /// humid, the less. 9 in the coldest and most humid, to
    /// [`RAINS_ABOVE_AT_THE_MOST`].
    pub const fn rains_above(self) -> u32 {
        let above = RAINS_ABOVE_IN_COLD_DRY_AIR + self.temperature / HEAT_A_LEVEL_MORE_HELD - self.humidity / HUMIDITY_A_LEVEL_SOONER;
        if above > RAINS_ABOVE_AT_THE_MOST { RAINS_ABOVE_AT_THE_MOST } else { above }
    }

    /// Whether a tile of this climate under `cloud` is raining.
    pub const fn rains(self, cloud: u32) -> bool {
        cloud > self.rains_above()
    }
}
