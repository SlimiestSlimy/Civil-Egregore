//! Terrain: heights settled by the seed and the cell alone, seamless
//! across superchunks, polygons joined by ramps; walls exactly where two cells beside
//! one another are more than a step apart in height.
//!
//! `cargo test`

use coordinates::{place_from_cartesian, CellCartesian, SuperchunkIndex, WORLD_MIDDLE};
use terrain::{height_shaped, wall, Shape, Terrain, STEP, WALLS};

/// The height of the cell `(x, y)` of a superchunk's `terrain`, from
/// its top left.
fn at(terrain: &Terrain, x: u32, y: u32) -> u16 {
    terrain.height(place_from_cartesian(x, y))
}

/// Whether `terrain` keeps a wall the `way`-th way at the cell `(x, y)`.
fn walled(terrain: &Terrain, way: usize, x: u32, y: u32) -> bool {
    terrain.walled(way, place_from_cartesian(x, y))
}

/// A shape of small polygons joined by cliffs: plenty of walls.
const CLIFFS: Shape = Shape { span: 8, levels: 40, edge: 2, ..Shape::DEFAULT };

/// The same seed gives the same heights, another seed others; a
/// superchunk's heights are the world's, whichever superchunk is made.
#[test]
fn heights_are_settled_by_the_seed_and_the_cell() {
    let superchunk = SuperchunkIndex::from_cartesian(2_000_000, 2_000_001);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let [first, again, other] = [7, 7, 8].map(|seed| Terrain::generate_shaped(&CLIFFS, seed, superchunk));
    assert!(first.heights == again.heights);
    assert!(first.heights != other.heights);
    for (x, y) in [(0, 0), (1023, 1023), (500, 3), (255, 256), (768, 511)] {
        assert_eq!(at(&first, x, y), height_shaped(&CLIFFS, 7, left + x, top + y));
    }
}

/// A wall is kept exactly where the two cells are more than a step
/// apart -- across the superchunk's edges too, by the heights beyond --
/// and with narrow edges some of the ground is walled, most of it not.
#[test]
fn walls_are_where_heights_are_more_than_a_step_apart() {
    let CellCartesian { x: left, y: top } = WORLD_MIDDLE.top_left().cartesian();
    let terrain = Terrain::generate_shaped(&CLIFFS, 1, WORLD_MIDDLE);
    let height = |seed, x, y| height_shaped(&CLIFFS, seed, x, y);
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
    assert!(share > 0.0005 && share < 0.25, "{:.3}% of steps walled: {counts:?}", 100.0 * share);
}

/// With broad edges the plains join by ramps and the shores fall
/// gently: no wall, and no two cells beside one another more than a
/// step apart.
#[test]
fn broad_edges_have_no_walls() {
    let terrain = Terrain::generate(1, WORLD_MIDDLE);
    assert_eq!(terrain.wall_counts(), [0; 2]);
    let heights: Vec<u16> = (0..1024).map(|x| at(&terrain, x, 500)).collect();
    assert!(heights.windows(2).all(|pair| pair[0].abs_diff(pair[1]) <= 1), "a row: {heights:?}");
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

/// The land as polygons: some ocean at the lowest ground, some plains
/// over the ocean's level, and between any two cells beside one
/// another -- a border crossed or not -- a ramp, never a jump.
#[test]
fn polygons_are_ocean_or_plains_joined_by_ramps() {
    let shape = Shape { span: 10, ..Shape::DEFAULT };
    let row: Vec<u64> = (0..40_000).map(|x| terrain::polygons::land(&shape, 1, 2_000_000_000 + x, 2_000_000_000)).collect();
    assert!(row.iter().all(|&high| (shape.ground as u64..=shape.ocean as u64 + 1 + shape.levels).contains(&high)));
    assert!(row.contains(&(shape.ground as u64)) && row.iter().any(|&high| high > shape.ocean as u64), "ocean and land both");
    assert!(row.windows(2).all(|pair| pair[0].abs_diff(pair[1]) <= 3), "no jump");
}
