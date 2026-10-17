//! The search in Morton order is the search over rows: the same area
//! put anywhere in the tiles comes to a goal as near, by a first step
//! that is one; and a set stepped a cell is its cells each stepped.
//!
//! `cargo test`

use pathfinding::morton::{self, bit_at, MortonWalls, Tiles, TILES_SIDE, TILES_WORDS};
use pathfinding::{rows_hold, step_towards, Cell, Rows, Walls, AREA_SIDE};
use utilities::rng::RandomSource;

/// `rows`, an area's, as tiles: the area's top left cell `across` and
/// `down` from the tiles'.
fn tiles_of(rows: &Rows, (across, down): (u32, u32)) -> Tiles {
    let mut tiles = [0; TILES_WORDS];
    for (x, y) in (0..AREA_SIDE as u32).flat_map(|y| (0..AREA_SIDE as u32).map(move |x| (x, y))).filter(|&(x, y)| rows[y as usize] >> x & 1 == 1) {
        let (word, bit) = bit_at(across + x, down + y);
        tiles[word] |= 1 << bit;
    }
    tiles
}

/// Rows drawn, about one cell in `one_in` set.
fn rows_drawn(random: &mut RandomSource, one_in: u64) -> Rows {
    std::array::from_fn(|_| (0..AREA_SIDE).fold(0, |row, x| row | u16::from(random.draw_below(one_in) == 0) << x))
}

/// A set of cells stepped along a coordinate is each of its cells
/// stepped, those past the tiles' edge gone.
#[test]
fn a_set_stepped_is_its_cells_stepped() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random = RandomSource::new(seed);
    for _ in 0..200 {
        let cells: Vec<(u32, u32)> = (0..40).map(|_| (random.draw_below(TILES_SIDE as u64) as u32, random.draw_below(TILES_SIDE as u64) as u32)).collect();
        let set = |cells: &mut dyn Iterator<Item = (i64, i64)>| {
            let mut tiles = [0u64; TILES_WORDS];
            for (x, y) in cells.filter(|&(x, y)| (0..TILES_SIDE as i64).contains(&x) && (0..TILES_SIDE as i64).contains(&y)) {
                let (word, bit) = bit_at(x as u32, y as u32);
                tiles[word] |= 1 << bit;
            }
            tiles
        };
        let here = set(&mut cells.iter().map(|&(x, y)| (x as i64, y as i64)));
        for (lane, (dx, dy)) in [(0, (1, 0)), (1, (0, 1))] {
            assert_eq!(morton::step_up(&here, lane), set(&mut cells.iter().map(|&(x, y)| (x as i64 + dx, y as i64 + dy))));
            assert_eq!(morton::step_down(&here, lane), set(&mut cells.iter().map(|&(x, y)| (x as i64 - dx, y as i64 - dy))));
        }
    }
}

/// An area searched in Morton order, wherever it is put in the tiles,
/// comes to a goal in the steps the search over rows does, by a first
/// step from which the goal is a step nearer.
#[test]
fn the_search_in_morton_order_is_the_search_over_rows() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random = RandomSource::new(seed);
    let (mut found, mut none) = (0, 0);
    for case in 0..3000 {
        let (passable, goals) = (rows_drawn(&mut random, 1 + case % 3).map(|row| !row), rows_drawn(&mut random, 20 + 30 * (case % 5)));
        let (east, south) = if case % 4 == 0 { ([0; AREA_SIDE], [0; AREA_SIDE]) } else { (rows_drawn(&mut random, 6), rows_drawn(&mut random, 6)) };
        let walls = Walls::new(east, south);
        let from = Cell { x: random.draw_below(AREA_SIDE as u64) as u8, y: random.draw_below(AREA_SIDE as u64) as u8 };
        let at = (random.draw_below(AREA_SIDE as u64 + 1) as u32, random.draw_below(AREA_SIDE as u64 + 1) as u32);
        let over_rows = step_towards(&passable, &walls, &goals, from, random.next_draw());
        let in_morton = morton::step_towards(&tiles_of(&passable, at), &MortonWalls::new(&tiles_of(&east, at), &tiles_of(&south, at)), &tiles_of(&goals, at), (at.0 + u32::from(from.x), at.1 + u32::from(from.y)), random.next_draw());
        assert_eq!(in_morton.map(|(_, steps)| steps), over_rows.map(|path| path.steps), "case {case}: the area at {at:?}, from {from:?}");
        let Some(((dx, dy), steps)) = in_morton else {
            none += 1;
            continue;
        };
        found += 1;
        let first = Cell { x: (from.x as i32 + dx) as u8, y: (from.y as i32 + dy) as u8 };
        assert!(!walls.bars_step(from, dx as i8, dy as i8), "case {case}: a first step through a wall");
        if steps == 1 {
            assert!(rows_hold(&goals, first), "case {case}: a step from a goal, and not onto it");
        } else {
            assert!(rows_hold(&passable, first) && step_towards(&passable, &walls, &goals, first, 0).is_some_and(|path| path.steps < steps), "case {case}: a first step no nearer");
        }
    }
    assert!(found > 500 && none > 100, "ways found, and none: {found}, {none}");
}
