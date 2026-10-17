//! Bitmaps laid out the way the encoding is meant for: streets on a
//! pitch, blocks between them, courtyards inside blocks, none of it
//! aligned to the quadtree (`docs/lab.md`, "`corpus/`").

use utilities::rng::RandomSource;
use bitmap::Bitmap;

/// How a city is laid out: how far apart the streets run, how wide
/// they are, how many courtyards a block is given and how big, and how
/// often a block is a park.
pub struct CityPlan {
    /// What a measurement calls it.
    pub display_name: &'static str,
    /// How far apart the streets run, in cells.
    pub street_pitch: i64,
    /// How wide a street is. A block is `pitch - street` across.
    pub street_width: i64,
    /// How many courtyards are cut out of each block.
    pub courtyards_per_block: u64,
    /// The sides a courtyard can have, in cells, each as likely.
    pub courtyard_sides: &'static [i64],
    /// How often, in a hundred blocks, one is left clear: a park.
    pub parks_in_a_hundred: u64,
    /// How many to measure over...
    pub timed_count: u64,
    /// ...and how many a test takes.
    pub tested_count: u64,
}

impl CityPlan {
    /// The side of one block, in cells.
    pub const fn block_side(&self) -> i64 {
        self.street_pitch - self.street_width
    }

    /// `count` bitmaps of this plan grown from `seed`, built one at a time.
    pub fn take_bitmaps(&'static self, seed: u64, count: u64) -> CityBitmaps {
        CityBitmaps { seed, bitmaps_left: count, city_plan: self }
    }

    /// As many as a timed run of this plan should take.
    pub fn timed_bitmaps(&'static self, seed: u64) -> CityBitmaps {
        self.take_bitmaps(seed, self.timed_count)
    }

    /// As many as a unit test of this plan should take.
    pub fn tested_bitmaps(&'static self, seed: u64) -> CityBitmaps {
        self.take_bitmaps(seed, self.tested_count)
    }
}

/// The layouts worth measuring on: blocks from a twelfth of the
/// bitmap down to a twentieth, and streets narrow and wide.
pub const CITY_PLANS: [CityPlan; 4] = [
    CityPlan {
        display_name: "blocks of 28, streets of 4",
        street_pitch: 32,
        street_width: 4,
        courtyards_per_block: 2,
        courtyard_sides: &[2, 4, 8],
        parks_in_a_hundred: 12,
        timed_count: 12,
        tested_count: 2,
    },
    CityPlan {
        display_name: "blocks of 24, streets of 8",
        street_pitch: 32,
        street_width: 8,
        courtyards_per_block: 3,
        courtyard_sides: &[2, 4, 8],
        parks_in_a_hundred: 12,
        timed_count: 12,
        tested_count: 2,
    },
    CityPlan {
        display_name: "blocks of 60, streets of 4",
        street_pitch: 64,
        street_width: 4,
        courtyards_per_block: 6,
        courtyard_sides: &[2, 4, 8],
        parks_in_a_hundred: 12,
        timed_count: 12,
        tested_count: 2,
    },
    CityPlan {
        display_name: "blocks of 12, streets of 4",
        street_pitch: 16,
        street_width: 4,
        courtyards_per_block: 1,
        courtyard_sides: &[2, 4, 8],
        parks_in_a_hundred: 12,
        timed_count: 12,
        tested_count: 2,
    },
];

/// A run of cities from consecutive seeds, built one at a time.
pub struct CityBitmaps {
    /// The next city's seed.
    seed: u64,
    /// How many cities are still to come.
    bitmaps_left: u64,
    /// What every city is laid out by.
    city_plan: &'static CityPlan,
}

impl Iterator for CityBitmaps {
    type Item = Bitmap;

    fn next(&mut self) -> Option<Bitmap> {
        if self.bitmaps_left == 0 {
            return None;
        }
        self.bitmaps_left -= 1;
        self.seed += 1;
        Some(one_laid_out(self.seed - 1, self.city_plan))
    }
}

/// One city: a grid of blocks with streets between them and
/// courtyards inside them, the grid starting at a random offset, so the
/// border cuts the blocks along it.
pub fn one_laid_out(seed: u64, city_plan: &CityPlan) -> Bitmap {
    let mut bitmap = Bitmap::new();
    let mut random_generator = RandomSource::new(seed);
    let (offset_x, offset_y) = (random_generator.draw_below(city_plan.street_pitch as u64) as i64, random_generator.draw_below(city_plan.street_pitch as u64) as i64);

    let mut y = offset_y - city_plan.street_pitch;
    while y < bitmap::HEIGHT as i64 {
        let mut x = offset_x - city_plan.street_pitch;
        while x < bitmap::WIDTH as i64 {
            lay_out_block(&mut bitmap, x, y, city_plan, &mut random_generator);
            x += city_plan.street_pitch;
        }
        y += city_plan.street_pitch;
    }
    bitmap
}

/// One block of `city_plan`, top left at `(x, y)`, filled, with courtyards
/// cut out of it -- or left clear altogether, which is a park.
fn lay_out_block(bitmap: &mut Bitmap, x: i64, y: i64, city_plan: &CityPlan, random_generator: &mut RandomSource) {
    if random_generator.percent_chance(city_plan.parks_in_a_hundred) {
        return;
    }
    let block_side_cells = city_plan.block_side();
    bitmap.set_rect(x, y, x + block_side_cells - 1, y + block_side_cells - 1);
    for _ in 0..city_plan.courtyards_per_block {
        let courtyard_side = city_plan.courtyard_sides[random_generator.draw_below(city_plan.courtyard_sides.len() as u64) as usize];
        if courtyard_side >= block_side_cells {
            continue;
        }
        let courtyard_room = (block_side_cells - courtyard_side + 1) as u64;
        let (courtyard_x, courtyard_y) = (x + random_generator.draw_below(courtyard_room) as i64, y + random_generator.draw_below(courtyard_room) as i64);
        bitmap.unset_rect(courtyard_x, courtyard_y, courtyard_x + courtyard_side - 1, courtyard_y + courtyard_side - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seed and a plan settle a bitmap, and nothing else does.
    #[test]
    fn a_seed_and_a_plan_settle_a_city() {
        for city_plan in &CITY_PLANS {
            let first_city = one_laid_out(7, city_plan);
            let second_city = one_laid_out(7, city_plan);
            for y in 0..=u8::MAX {
                for x in 0..=u8::MAX {
                    assert_eq!(first_city.get(x, y), second_city.get(x, y), "{} differs at ({x}, {y})", city_plan.display_name);
                }
            }
            assert_ne!(
                first_city.count_set(),
                one_laid_out(8, city_plan).count_set(),
                "{} gives the same bitmap for two seeds",
                city_plan.display_name
            );
        }
    }

    /// Every plan puts something on the bitmap and leaves something
    /// off it, or it is not measuring anything.
    #[test]
    fn every_plan_lays_out_a_city() {
        for city_plan in &CITY_PLANS {
            let bitmap = one_laid_out(0, city_plan);
            let set_cell_count = bitmap.count_set();
            assert!(set_cell_count > 0, "{} lays out nothing", city_plan.display_name);
            assert!(set_cell_count < 65536, "{} covers everything", city_plan.display_name);
            assert!(city_plan.block_side() > 0, "{} has no block", city_plan.display_name);
        }
    }

    /// The blocks land on the quadtree's grid, which is what these
    /// are for.
    #[test]
    fn a_plan_lands_on_the_quadtrees_grid() {
        for city_plan in &CITY_PLANS {
            assert!((city_plan.street_pitch as u64).is_power_of_two(), "{} has a pitch off the grid", city_plan.display_name);
            assert!((city_plan.street_width as u64).is_power_of_two(), "{} has a street off the grid", city_plan.display_name);
        }
    }
}
