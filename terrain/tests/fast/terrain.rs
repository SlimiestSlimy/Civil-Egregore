//! Terrain: heights settled by the seed and the cell alone, seamless
//! across superchunks, rolling; walls exactly where two cells beside
//! one another are more than a step apart in height.
//!
//! `cargo test`

use coordinates::{place_from_cartesian, CellCartesian, SuperchunkIndex, WORLD_MIDDLE};
use terrain::{height, height_shaped, wall, Shape, Terrain, STEP, WALLS};

/// The height of the cell `(x, y)` of a superchunk's `terrain`, from
/// its top left.
fn at(terrain: &Terrain, x: u32, y: u32) -> u16 {
    terrain.height(place_from_cartesian(x, y))
}

/// Whether `terrain` keeps a wall the `way`-th way at the cell `(x, y)`.
fn walled(terrain: &Terrain, way: usize, x: u32, y: u32) -> bool {
    terrain.walled(way, place_from_cartesian(x, y))
}

/// The same seed gives the same heights, another seed others; a
/// superchunk's heights are the world's, whichever superchunk is made.
#[test]
fn heights_are_settled_by_the_seed_and_the_cell() {
    let superchunk = SuperchunkIndex::from_cartesian(2_000_000, 2_000_001);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let (first, again, other) = (Terrain::generate(7, superchunk), Terrain::generate(7, superchunk), Terrain::generate(8, superchunk));
    assert!(first.heights == again.heights);
    assert!(first.heights != other.heights);
    for (x, y) in [(0, 0), (1023, 1023), (500, 3), (255, 256), (768, 511)] {
        assert_eq!(at(&first, x, y), height(7, left + x, top + y));
    }
}

/// A shape all hills: level land well over the ocean.
const HILLS: Shape = Shape { weights: [168, 22, 61, 4], ocean: 0, coast: 1, ground: 512, rise: 0, rise_span: 14 };
/// A shape all ocean: the land rises, never to the ocean's level.
const OCEAN: Shape = Shape { weights: [168, 22, 61, 4], ocean: 4096, coast: 64, ground: 256, rise: 1024, rise_span: 14 };

/// The hills roll: heights span most of their range over a
/// superchunk, and no cell is far from its neighbour's.
#[test]
fn the_hills_roll() {
    let terrain = Terrain::generate_shaped(&HILLS, 1, WORLD_MIDDLE);
    let (mut low, mut high, mut steepest) = (u16::MAX, 0, 0);
    for y in 0..1024 {
        for x in 0..1024 {
            let here = at(&terrain, x, y);
            (low, high) = (low.min(here), high.max(here));
            if x > 0 {
                steepest = steepest.max(here.abs_diff(at(&terrain, x - 1, y)));
            }
        }
    }
    assert!(high - low > 80, "from {low} to {high}");
    assert!(steepest <= 8, "a step of {steepest} between two cells");
}

/// A wall is kept exactly where the two cells are more than a step
/// apart -- across the superchunk's edges too, by the heights beyond --
/// and some of the ground is walled, most of it not.
#[test]
fn walls_are_where_heights_are_more_than_a_step_apart() {
    let CellCartesian { x: left, y: top } = WORLD_MIDDLE.top_left().cartesian();
    let terrain = Terrain::generate_shaped(&HILLS, 1, WORLD_MIDDLE);
    let height = |seed, x, y| height_shaped(&HILLS, seed, x, y);
    for y in (0..1024).step_by(7).chain([1023]) {
        for x in (0..1024).step_by(5).chain([0, 1023]) {
            for (way, &(_, (dx, dy))) in WALLS.iter().enumerate() {
                let (here, there) = (height(1, left + x, top + y), height(1, (left + x).wrapping_add_signed(dx), (top + y).wrapping_add_signed(dy)));
                assert_eq!(walled(&terrain, way, x, y), here.abs_diff(there) > STEP, "({x}, {y}) way {way}: {here} and {there}");
                assert_eq!(wall(here, there), here.abs_diff(there) > STEP);
            }
        }
    }
    let counts = terrain.wall_counts();
    let share = counts.iter().sum::<u64>() as f64 / (2.0 * 1024.0 * 1024.0);
    assert!(share > 0.002 && share < 0.25, "{:.2}% of steps walled: {counts:?}", 100.0 * share);
}

/// The land's rise is gentle: under the ocean, where no hill stands,
/// no wall, and no two cells beside one another more than a step apart.
#[test]
fn the_oceans_floor_has_no_walls() {
    let terrain = Terrain::generate_shaped(&OCEAN, 1, WORLD_MIDDLE);
    assert_eq!(terrain.wall_counts(), [0; 2]);
    let heights: Vec<u16> = (0..1024).map(|x| at(&terrain, x, 500)).collect();
    assert!(heights.windows(2).all(|pair| pair[0].abs_diff(pair[1]) <= 1), "a row of the floor: {heights:?}");
}

/// Flat ground has no walls; a cliff has them along it.
#[test]
fn a_cliff_is_walled_along_its_length() {
    assert_eq!(Terrain::from_heights(|_, _| 9).wall_counts(), [0; 2]);
    // Ground 3 higher from column 500 on.
    let terrain = Terrain::from_heights(|x, _| if x >= 500 { 3 } else { 0 });
    assert_eq!(terrain.wall_counts(), [1024, 0], "east walls down it, none south");
    assert!(walled(&terrain, 0, 499, 77));
    assert!(!walled(&terrain, 0, 500, 77) && !walled(&terrain, 0, 498, 77));
}
