//! The search in Morton order: over [`Tiles`], the 32x32 cells of four
//! aligned tiles of 16 two by two, as they lie in their bitmaps -- 16
//! words, a word an 8x8 word tile, the cell `x` across and `y` down at
//! the bit whose index is the two interleaved. A set stepped a cell
//! east is three masked shifts a word and a carry into the word tile
//! beside; a wave spreads by eight such steps
//! (`docs/pathfinding.md`, "In Morton order").

/// Words of [`Tiles`]: 4 by 4 word tiles.
pub const TILES_WORDS: usize = 16;
/// Cells along the side of [`Tiles`].
pub const TILES_SIDE: u32 = 32;
/// 32x32 cells, a bit each, in Morton order.
pub type Tiles = [u64; TILES_WORDS];

/// The bits of `0..64` whose index, under `mask`, is `value`.
const fn bits_whose_index_masks_to(mask: u32, value: u32) -> u64 {
    let (mut found, mut place) = (0, 0);
    while place < u64::BITS {
        if place & mask == value {
            found |= 1 << place;
        }
        place += 1;
    }
    found
}

/// A coordinate's three bits of a place in a word tile: `lane` 0 for
/// x, the even ones, 1 for y.
const fn coordinate_bits(lane: u32) -> [u32; 3] {
    [1 << lane, 4 << lane, 16 << lane]
}

/// For each coordinate, the cells of a word tile by how a step up it
/// carries: those whose lowest bit of it is 0, those whose lowest is 1
/// and next 0, those whose two lowest are 1 and third 0 -- each moved
/// by one shift -- and those on the tile's far edge, which leave it.
const STEP_UP_KINDS: [[u64; 4]; 2] = {
    let mut up = [[0; 4]; 2];
    let mut lane = 0;
    while lane < 2 {
        let [low, middle, high] = coordinate_bits(lane);
        up[lane as usize] = [bits_whose_index_masks_to(low, 0), bits_whose_index_masks_to(low | middle, low), bits_whose_index_masks_to(low | middle | high, low | middle), bits_whose_index_masks_to(low | middle | high, low | middle | high)];
        lane += 1;
    }
    up
};

/// [`STEP_UP_KINDS`], for a step down the coordinate: a borrow where it carried.
const STEP_DOWN_KINDS: [[u64; 4]; 2] = {
    let mut down = [[0; 4]; 2];
    let mut lane = 0;
    while lane < 2 {
        let [low, middle, high] = coordinate_bits(lane);
        down[lane as usize] = [bits_whose_index_masks_to(low, low), bits_whose_index_masks_to(low | middle, middle), bits_whose_index_masks_to(low | middle | high, high), bits_whose_index_masks_to(low | middle | high, 0)];
        lane += 1;
    }
    down
};

/// How far a cell of each of [`STEP_UP_KINDS`]'s kinds moves in its word, for x:
/// the bit gained less the bits lost. Twice as far for y.
const KIND_SHIFTS: [u32; 4] = [1, 3, 11, 21];

/// The index of the word tile `x` across and `y` down, each 0 to 3.
const fn word_tile_at(x: usize, y: usize) -> usize {
    x & 1 | (x & 2) << 1 | (y & 1) << 1 | (y & 2) << 2
}

/// A coordinate of a cell of [`Tiles`], its five bits spread to every
/// other bit.
const fn spread_coordinate_bits(of: u32) -> u32 {
    of & 1 | (of & 2) << 1 | (of & 4) << 2 | (of & 8) << 3 | (of & 16) << 4
}

/// For each coordinate, the word tile a step up it from each leads
/// into, and a step down: [`TILES_WORDS`] where there is none.
const WORD_TILE_BESIDE: [[[usize; TILES_WORDS]; 2]; 2] = {
    let mut beside = [[[TILES_WORDS; TILES_WORDS]; 2]; 2];
    let mut tile = 0;
    while tile < TILES_WORDS {
        // A word tile's index: its x in bits 0 and 2, its y in bits 1 and 3.
        let (x, y) = (tile & 1 | tile >> 1 & 2, tile >> 1 & 1 | tile >> 2 & 2);
        if x < 3 {
            beside[0][0][tile] = word_tile_at(x + 1, y);
        }
        if x > 0 {
            beside[0][1][tile] = word_tile_at(x - 1, y);
        }
        if y < 3 {
            beside[1][0][tile] = word_tile_at(x, y + 1);
        }
        if y > 0 {
            beside[1][1][tile] = word_tile_at(x, y - 1);
        }
        tile += 1;
    }
    beside
};

/// `cells` each a cell further along a coordinate -- `lane` 0 east, 1
/// south: those past the tiles' edge gone.
#[inline(always)]
pub fn step_up(cells: &Tiles, lane: usize) -> Tiles {
    let mut moved = [0u64; TILES_WORDS + 1];
    let (kinds, shift) = (&STEP_UP_KINDS[lane], lane as u32);
    for (tile, &word) in cells.iter().enumerate() {
        moved[tile] |= (word & kinds[0]) << (KIND_SHIFTS[0] << shift) | (word & kinds[1]) << (KIND_SHIFTS[1] << shift) | (word & kinds[2]) << (KIND_SHIFTS[2] << shift);
        moved[WORD_TILE_BESIDE[lane][0][tile]] |= (word & kinds[3]) >> (KIND_SHIFTS[3] << shift);
    }
    std::array::from_fn(|tile| moved[tile])
}

/// `cells` each a cell back along a coordinate -- `lane` 0 west, 1
/// north: those past the tiles' edge gone.
#[inline(always)]
pub fn step_down(cells: &Tiles, lane: usize) -> Tiles {
    let mut moved = [0u64; TILES_WORDS + 1];
    let (kinds, shift) = (&STEP_DOWN_KINDS[lane], lane as u32);
    for (tile, &word) in cells.iter().enumerate() {
        moved[tile] |= (word & kinds[0]) >> (KIND_SHIFTS[0] << shift) | (word & kinds[1]) >> (KIND_SHIFTS[1] << shift) | (word & kinds[2]) >> (KIND_SHIFTS[2] << shift);
        moved[WORD_TILE_BESIDE[lane][1][tile]] |= (word & kinds[3]) << (KIND_SHIFTS[3] << shift);
    }
    std::array::from_fn(|tile| moved[tile])
}

/// The cells of `cells` that `other` has not.
#[inline(always)]
fn cells_less(cells: &Tiles, other: &Tiles) -> Tiles {
    std::array::from_fn(|tile| cells[tile] & !other[tile])
}

/// The cells of either.
#[inline(always)]
fn cells_of_either(cells: &Tiles, other: &Tiles) -> Tiles {
    std::array::from_fn(|tile| cells[tile] | other[tile])
}

/// The bit of the cell `x` across and `y` down in [`Tiles`]: its word,
/// and its bit there.
pub const fn bit_at(x: u32, y: u32) -> (usize, u32) {
    let index = spread_coordinate_bits(x) | spread_coordinate_bits(y) << 1;
    ((index / u64::BITS) as usize, index % u64::BITS)
}

/// The walls between the cells of [`Tiles`], as what each step of a
/// spread must leave behind: the cells that may not step each way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MortonWalls {
    /// Cells that may not step east, west, south-east and north-east --
    /// each a set a step east or west is taken from.
    no_step_across: [Tiles; 6],
    /// Cells that may not step south, and north.
    no_south: Tiles,
    /// ...north.
    no_north: Tiles,
    /// Whether there is any wall at all.
    any_wall: bool,
}

impl MortonWalls {
    /// The walls `east` and `south` of the cells: a cell's step down
    /// and east is barred by a wall east of it or of the cell below, or
    /// south of it or of the cell east -- its two ways round; down and
    /// west, likewise to the west. A step up is its opposite's down.
    pub fn new(east: &Tiles, south: &Tiles) -> Self {
        let any_wall = east.iter().chain(south).any(|&word| word != 0);
        let south_east = cells_of_either(&cells_of_either(east, &step_down(east, 1)), &cells_of_either(south, &step_down(south, 0)));
        let east_of_east = step_up(east, 0);
        let south_west = cells_of_either(&cells_of_either(&east_of_east, &step_down(&east_of_east, 1)), &cells_of_either(south, &step_up(south, 0)));
        // A step up or west is barred by the wall its target keeps: the target's, stepped back onto who steps.
        let (below_south_east, below_south_west) = (step_up(&south_east, 1), step_up(&south_west, 1));
        Self { no_step_across: [*east, east_of_east, south_east, south_west, step_up(&below_south_east, 0), step_down(&below_south_west, 0)], no_south: *south, no_north: step_up(south, 1), any_wall }
    }

    /// `cells` and every neighbour of them no wall is before.
    #[inline]
    pub fn spread_through(&self, cells: &Tiles) -> Tiles {
        if !self.any_wall {
            let across = cells_of_either(cells, &cells_of_either(&step_up(cells, 0), &step_down(cells, 0)));
            return cells_of_either(&across, &cells_of_either(&step_up(&across, 1), &step_down(&across, 1)));
        }
        let [no_east, no_west, no_south_east, no_south_west, no_north_west, no_north_east] = &self.no_step_across;
        let (east, west) = (step_up(&cells_less(cells, no_east), 0), step_down(&cells_less(cells, no_west), 0));
        // South of it, and the diagonals down: stepped east or west first, then all down at once.
        let downwards = cells_of_either(&cells_less(cells, &self.no_south), &cells_of_either(&step_up(&cells_less(cells, no_south_east), 0), &step_down(&cells_less(cells, no_south_west), 0)));
        let upwards = cells_of_either(&cells_less(cells, &self.no_north), &cells_of_either(&step_down(&cells_less(cells, no_north_west), 0), &step_up(&cells_less(cells, no_north_east), 0)));
        cells_of_either(&cells_of_either(cells, &cells_of_either(&east, &west)), &cells_of_either(&step_up(&downwards, 1), &step_down(&upwards, 1)))
    }
}

/// The eight neighbours of a cell, as steps across and down, in the
/// Morton order of where they lie about it.
const NEIGHBOUR_STEPS: [(i32, i32); 8] = [(-1, -1), (0, -1), (-1, 0), (1, -1), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// A walker on the cell `(x, y)`'s next step -- across and down, each
/// -1 to 1 -- to the nearest cell of `goals` over the cells of
/// `passable`, through no wall of `walls`, and how many steps away that
/// goal is; `None` if none can be walked to. Of several first steps as
/// short, the one `pick` falls on, counted in [`NEIGHBOUR_STEPS`]' order.
pub fn step_towards(passable: &Tiles, walls: &MortonWalls, goals: &Tiles, (x, y): (u32, u32), pick: u64) -> Option<((i32, i32), u8)> {
    let (word, bit) = bit_at(x, y);
    let mut from = [0u64; TILES_WORDS];
    from[word] = 1 << bit;
    let beside = cells_less(&walls.spread_through(&from), &from);
    let mut reached = cells_less(goals, &from);
    let mut steps = 1;
    loop {
        let arrived: Tiles = std::array::from_fn(|tile| beside[tile] & reached[tile]);
        if arrived.iter().any(|&word| word != 0) {
            let held = |&(dx, dy): &(i32, i32)| {
                let (word, bit) = bit_at((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                arrived[word] >> bit & 1 == 1
            };
            let count = NEIGHBOUR_STEPS.iter().filter(|step| held(step)).count() as u64;
            return NEIGHBOUR_STEPS.iter().filter(|step| held(step)).nth((pick % count) as usize).map(|&step| (step, steps));
        }
        let wider = walls.spread_through(&reached);
        let mut grew = 0;
        for tile in 0..TILES_WORDS {
            let new = wider[tile] & passable[tile] & !reached[tile];
            reached[tile] |= new;
            grew |= new;
        }
        if grew == 0 {
            return None;
        }
        steps += 1;
    }
}
