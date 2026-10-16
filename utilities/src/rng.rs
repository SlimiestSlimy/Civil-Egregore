//! Civil Egregore's one random source: SplitMix64, seeded, whose whole state
//! is one word, so whatever draws from it -- a test bitmap, a search --
//! is settled by its seed alone, on every run and every machine.

use crate::chance::Chance;
use crate::hash::{mix_word, GOLDEN_RATIO, MIX_1};

/// The random source.
pub struct RandomSource(
    /// Its whole state: the seed, before the first draw.
    u64,
);

impl RandomSource {
    /// A source settled by `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// A source of its own for `stream` -- a superchunk index, say -- of
    /// `seed`: its state is mixed, not `seed` moved along, so
    /// two streams are not one sequence a few draws apart, as two
    /// sources whose seeds differ by a multiple of the step would be.
    pub fn for_stream(seed: u64, stream: u64) -> Self {
        let mut stream_mixer = Self(seed ^ stream.wrapping_mul(MIX_1));
        let first_draw = stream_mixer.next_draw();
        Self(first_draw ^ stream_mixer.next_draw().rotate_left(32))
    }

    /// Its whole state: a generator made from it draws what this one
    /// would have. What a save keeps.
    pub fn whole_state(&self) -> u64 {
        self.0
    }

    /// The next draw: any 64-bit value.
    #[inline]
    pub fn next_draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GOLDEN_RATIO);
        mix_word(self.0)
    }

    /// A number in `0..bound`.
    pub fn draw_below(&mut self, exclusive_bound: u64) -> u64 {
        self.next_draw() % exclusive_bound
    }

    /// A number in `low_inclusive..=high_inclusive`.
    pub fn draw_between(&mut self, low_inclusive: u64, high_inclusive: u64) -> u64 {
        low_inclusive + self.draw_below(high_inclusive - low_inclusive + 1)
    }

    /// True `percent_true` times in a hundred.
    pub fn percent_chance(&mut self, percent_true: u64) -> bool {
        self.draw_below(100) < percent_true
    }

    /// True with `chance`: the draw's high 32 bits under its parts.
    #[inline]
    pub fn chance(&mut self, chance: Chance) -> bool {
        (self.next_draw() >> 32) < chance.parts()
    }

    /// True `part` times in `whole_chance`: which of two things it is that
    /// happens with `whole_chance`, one of them with `part`. `whole_chance` is not
    /// never.
    #[inline]
    pub fn chance_among(&mut self, part: Chance, whole_chance: Chance) -> bool {
        self.draw_below(whole_chance.parts()) < part.parts()
    }

    /// A number in `[0, 1)`: a float, for what is not the simulation --
    /// a test bitmap, a search. Nothing a world follows from draws one.
    pub fn draw_unit_float(&mut self) -> f64 {
        (self.next_draw() >> (u64::BITS - f64::MANTISSA_DIGITS)) as f64 / (1u64 << f64::MANTISSA_DIGITS) as f64
    }
}
