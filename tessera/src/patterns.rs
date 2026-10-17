//! The pattern pyramid: a number per tile, whole bitmap to 4x4, equal for
//! two tiles of one size exactly when they hold the same cells -- so
//! whether a tile is homogeneous, and whether it is copyable, is one
//! comparison. `docs/tessera.md`, "The pattern pyramid".
//!
//! Function by function: `docs/reference.md`, "`patterns.rs`".

use crate::tile::{cells_in_tile, copy_offset, tiles_in_level, TilePyramid, Tile, FLOOR_LEVEL};
use utilities::bitmap::Bitmap;

/// Every cell of the tile clear.
pub const ALL_CLEAR: u16 = 0;
/// Every cell of the tile set.
pub const ALL_SET: u16 = 1;
/// The first number a pattern that is not homogeneous gets.
const FIRST_PATTERN: u16 = 2;

/// What a tile whose pattern number is `pattern_number` holds, if every cell of
/// it agrees.
#[inline]
pub fn homogeneous_value_of(pattern_number: u16) -> Option<bool> {
    match pattern_number {
        ALL_CLEAR => Some(false),
        ALL_SET => Some(true),
        _ => None,
    }
}

/// Levels numbered: the whole bitmap to the 4x4 floor.
const LEVEL_COUNT: usize = FLOOR_LEVEL as usize + 1;
/// A 4x4's cells: its pattern, one run of the bitmap.
const FLOOR_CELLS: usize = cells_in_tile(FLOOR_LEVEL);
const _: () = assert!(tiles_in_level(FLOOR_LEVEL) + FIRST_PATTERN as usize <= 1 << u16::BITS, "every number fits a u16");
/// Bits a number takes in a coarser tile's pattern.
const NUMBER_BITS: usize = u16::BITS as usize;
/// A coarser tile's pattern when all four children are clear...
const CHILDREN_ALL_CLEAR: u64 = 0;
/// ...and when all four are set.
const CHILDREN_ALL_SET: u64 = 0x0001_0001_0001_0001 * ALL_SET as u64;

/// Where each level's slots start, and where the finest one's end: a
/// power of two for each level, at least twice its tiles, so a probe
/// ends soon.
const SLOT_STARTS: [usize; LEVEL_COUNT + 1] = {
    let mut slot_starts = [0; LEVEL_COUNT + 1];
    let mut level_index = 0;
    while level_index < LEVEL_COUNT {
        slot_starts[level_index + 1] = slot_starts[level_index] + (2 * tiles_in_level(level_index as u8)).next_power_of_two();
        level_index += 1;
    }
    slot_starts
};
/// Where each level's numbers start among every level's, and where the
/// finest one's end: one for each of its tiles, and the two homogeneous
/// ones.
const NUMBER_STARTS: [usize; LEVEL_COUNT + 1] = {
    let mut number_starts = [0; LEVEL_COUNT + 1];
    let mut level_index = 0;
    while level_index < LEVEL_COUNT {
        number_starts[level_index + 1] = number_starts[level_index] + tiles_in_level(level_index as u8) + FIRST_PATTERN as usize;
        level_index += 1;
    }
    number_starts
};
/// Every level's numbers together.
const NUMBER_COUNT: usize = NUMBER_STARTS[LEVEL_COUNT];

/// An empty slot: no pattern that is not homogeneous has number 0.
const EMPTY_SLOT: u16 = ALL_CLEAR;

/// Every tile's pattern number, and the tables that hand them out.
pub struct PatternPyramid {
    /// Every tile's number.
    pattern_numbers: TilePyramid<u16, FLOOR_LEVEL>,
    /// Every level's hash table, one after another.
    slots: Box<[u16]>,
    /// For every level and number, the Morton index of the tile it first
    /// appeared at.
    first_tile: Box<[u16]>,
    /// How many numbers each level has handed out.
    handed_out: [u16; LEVEL_COUNT],
    /// For every level and number, whether a second tile has it.
    repeated_numbers: [u64; NUMBER_COUNT.div_ceil(u64::BITS as usize)],
}

impl PatternPyramid {
    /// Every table allocated, nothing numbered.
    pub fn new() -> Self {
        Self {
            pattern_numbers: TilePyramid::new(),
            slots: vec![EMPTY_SLOT; SLOT_STARTS[LEVEL_COUNT]].into_boxed_slice(),
            first_tile: vec![0; NUMBER_COUNT].into_boxed_slice(),
            handed_out: [FIRST_PATTERN; LEVEL_COUNT],
            repeated_numbers: [0; NUMBER_COUNT.div_ceil(u64::BITS as usize)],
        }
    }

    /// Numbers every tile of `bitmap`, finest level first.
    pub fn number_every_tile(&mut self, bitmap: &Bitmap) {
        self.slots.fill(EMPTY_SLOT);
        self.handed_out = [FIRST_PATTERN; LEVEL_COUNT];
        self.repeated_numbers.fill(0);
        for tile_level in (0..=FLOOR_LEVEL).rev() {
            for tile_place in 0..tiles_in_level(tile_level) {
                let pattern_number = self.number_for(bitmap, tile_level, tile_place);
                self.pattern_numbers.set_at(tile_level, tile_place, pattern_number);
            }
        }
    }

    /// The pattern of `level_index`'s tile at Morton index `tile_place`: a 4x4's
    /// cells, or a coarser tile's four children's numbers.
    #[inline]
    fn pattern_key(&self, bitmap: &Bitmap, tile_level: u8, tile_place: usize) -> u64 {
        if tile_level == FLOOR_LEVEL {
            return bitmap.morton_run(tile_place * FLOOR_CELLS, FLOOR_CELLS);
        }
        let child_numbers = self.pattern_numbers.children_at(tile_level, tile_place);
        (0..4).fold(0, |key_so_far, child_index| key_so_far | (child_numbers[child_index] as u64) << (child_index * NUMBER_BITS))
    }

    /// The number of `level_index`'s tile at Morton index `tile_place`: a
    /// homogeneous pattern's own, one already handed out, or the next.
    #[inline]
    fn number_for(&mut self, bitmap: &Bitmap, tile_level: u8, tile_place: usize) -> u16 {
        let tile_key = self.pattern_key(bitmap, tile_level, tile_place);
        let (all_clear, all_set) = if tile_level == FLOOR_LEVEL { (0, (1 << FLOOR_CELLS) - 1) } else { (CHILDREN_ALL_CLEAR, CHILDREN_ALL_SET) };
        if tile_key == all_clear {
            return ALL_CLEAR;
        }
        if tile_key == all_set {
            return ALL_SET;
        }
        let level_index = tile_level as usize;
        let (first_slot, slot_count) = (SLOT_STARTS[level_index], SLOT_STARTS[level_index + 1] - SLOT_STARTS[level_index]);
        let first_number = NUMBER_STARTS[level_index];
        let mut probe_slot = utilities::hash::slot(tile_key, slot_count);
        loop {
            let slot_number = self.slots[first_slot + probe_slot];
            if slot_number == EMPTY_SLOT {
                let new_number = self.handed_out[level_index];
                self.handed_out[level_index] += 1;
                self.slots[first_slot + probe_slot] = new_number;
                self.first_tile[first_number + new_number as usize] = tile_place as u16;
                return new_number;
            }
            let number_entry = first_number + slot_number as usize;
            if self.pattern_key(bitmap, tile_level, self.first_tile[number_entry] as usize) == tile_key {
                self.repeated_numbers[number_entry / u64::BITS as usize] |= 1 << (number_entry % u64::BITS as usize);
                return slot_number;
            }
            probe_slot = (probe_slot + 1) & (slot_count - 1);
        }
    }

    /// `tile`'s pattern number.
    #[inline]
    pub fn pattern_number(&self, tile: Tile) -> u16 {
        self.pattern_numbers.get(tile)
    }

    /// `tile`'s four children's pattern numbers, in reading order.
    #[inline]
    pub fn children_numbers(&self, tile: Tile) -> [u16; 4] {
        self.pattern_numbers.child_elements(tile)
    }

    /// Whether another tile of `level_index` holds the pattern numbered
    /// `pattern_number` -- always, for a homogeneous one: only then can a tile
    /// with it be a copy.
    #[inline]
    pub fn pattern_repeats(&self, tile_level: u8, pattern_number: u16) -> bool {
        let number_entry = NUMBER_STARTS[tile_level as usize] + pattern_number as usize;
        pattern_number < FIRST_PATTERN || self.repeated_numbers[number_entry / u64::BITS as usize] >> (number_entry % u64::BITS as usize) & 1 == 1
    }

    /// The first direction, near offsets before far, whose copy of
    /// `tile`, whose number is `tile_number`, holds the same cells, if any.
    pub fn copy_source(&self, tile: Tile, tile_number: u16) -> Option<(bool, u8)> {
        if !self.pattern_repeats(tile.tile_level, tile_number) {
            return None;
        }
        [false, true].into_iter().find_map(|is_far_copy| {
            (0..crate::tile::COPY_DIRECTIONS)
                .find(|&copy_direction| tile.offset_by(copy_offset(is_far_copy, copy_direction)).is_some_and(|source_tile| self.pattern_number(source_tile) == tile_number))
                .map(|copy_direction| (is_far_copy, copy_direction))
        })
    }
}
