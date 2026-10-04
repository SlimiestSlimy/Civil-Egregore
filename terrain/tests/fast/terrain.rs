//! Terrain: heights settled by the seed and the cell alone, seamless
//! across superchunks, polygons joined by ramps; walls exactly where two cells beside
//! one another are more than a step apart in height.
//!
//! `cargo test`

use coordinates::{place_from_cartesian, CellCartesian, SuperchunkIndex, WORLD_MIDDLE};
use terrain::{height_shaped, wall, Shape, Terrain, STEP, WALLS};

/// The seed the run's tests grow their ground from: the crate's,
/// rolled every few runs (`utilities::seed`), so that nothing passes on
/// one seed alone.
fn seed() -> u64 {
    utilities::seed::counted()
}

/// The height of the cell `(x, y)` of a superchunk's `terrain`, from
/// its top left.
fn at(terrain: &Terrain, x: u32, y: u32) -> u16 {
    terrain.height(place_from_cartesian(x, y))
}

/// Whether `terrain` keeps a wall the `way`-th way at the cell `(x, y)`.
fn walled(terrain: &Terrain, way: usize, x: u32, y: u32) -> bool {
    terrain.walled(way, place_from_cartesian(x, y))
}

/// A shape of land and ocean little apart in height, joined by lines
/// that slope their whole length, and nothing finer: no walls.
const RAMPS: Shape = Shape { ground: 461, highest: 561, narrow: 1 << 16, wide: 1 << 16, soft: 512, hard: 512, finer_depth: 0, ..Shape::DEFAULT };
/// A shape all land, of small triangles joined by cliffs: plenty of walls.
const CLIFFS: Shape = Shape { span: 8, sea: 0, highest: 552, narrow: 2, wide: 2, finer_depth: 3, ..Shape::DEFAULT };

/// The same seed gives the same heights, another seed others; a
/// superchunk's heights are the world's, whichever superchunk is made.
#[test]
fn heights_are_settled_by_the_seed_and_the_cell() {
    let superchunk = SuperchunkIndex::from_cartesian(2_000_000, 2_000_001);
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let [first, again, other] = [seed(), seed(), seed() + 1].map(|seed| Terrain::generate_shaped(&CLIFFS, seed, superchunk));
    assert!(first.heights == again.heights);
    assert!(first.heights != other.heights);
    for (x, y) in [(0, 0), (1023, 1023), (500, 3), (255, 256), (768, 511)] {
        assert_eq!(at(&first, x, y), height_shaped(&CLIFFS, seed(), left + x, top + y));
    }
}

/// A wall is kept exactly where the two cells are more than a step
/// apart -- across the superchunk's edges too, by the heights beyond --
/// and with narrow edges some of the ground is walled, most of it not.
#[test]
fn walls_are_where_heights_are_more_than_a_step_apart() {
    let CellCartesian { x: left, y: top } = WORLD_MIDDLE.top_left().cartesian();
    let terrain = Terrain::generate_shaped(&CLIFFS, seed(), WORLD_MIDDLE);
    let height = |x, y| height_shaped(&CLIFFS, seed(), x, y);
    for y in (0..1024).step_by(7).chain([1023]) {
        for x in (0..1024).step_by(5).chain([0, 1023]) {
            for (way, &(_, (dx, dy))) in WALLS.iter().enumerate() {
                let (here, there) = (height(left + x, top + y), height((left + x).wrapping_add_signed(dx), (top + y).wrapping_add_signed(dy)));
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
    let terrain = Terrain::generate_shaped(&RAMPS, seed(), WORLD_MIDDLE);
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

/// The land as a mesh: some ocean at the lowest ground, some land over
/// the ocean's level, and between any two cells beside one another --
/// a line crossed or not -- a slope, never a break.
#[test]
fn the_mesh_is_ocean_or_land_joined_by_slopes() {
    let shape = Shape { span: 10, ..RAMPS };
    let row: Vec<u64> = (0..40_000).map(|x| terrain::mesh::land(&shape, seed(), 2_000_000_000 + x, 2_000_000_000)).collect();
    assert!(row.iter().all(|&high| (shape.ground..=shape.highest.max(shape.ocean + 1)).contains(&(high as u16))));
    assert!(row.contains(&(shape.ground as u64)) && row.iter().any(|&high| high > shape.ocean as u64), "ocean and land both");
    assert!(row.windows(2).all(|pair| pair[0].abs_diff(pair[1]) <= 10), "no break");
}

/// A cell's height is the same in whatever order cells are asked for:
/// one after another along a row, back along it, each alone, or down
/// columns -- cliffs, finer meshes and all.
#[test]
fn heights_are_the_same_in_whatever_order_they_are_asked_for() {
    use terrain::mesh::Lands;
    let (left, top, side) = (2_147_000_000u32, 2_147_100_000u32, 300u32);
    let cells = || (0..side * side).map(|cell| (left + cell % side * 7, top + cell / side * 7));
    let mut lands = Lands::new(&CLIFFS, seed());
    let forwards: Vec<u16> = cells().map(|(x, y)| lands.height(x, y)).collect();
    let mut lands = Lands::new(&CLIFFS, seed());
    let mut backwards: Vec<u16> = cells().collect::<Vec<_>>().into_iter().rev().map(|(x, y)| lands.height(x, y)).collect();
    backwards.reverse();
    let mut lands = Lands::new(&CLIFFS, seed());
    let mut columns = vec![0; forwards.len()];
    for cell in 0..side * side {
        let (across, down) = (cell / side, cell % side);
        columns[(down * side + across) as usize] = lands.height(left + across * 7, top + down * 7);
    }
    let alone: Vec<u16> = cells().map(|(x, y)| height_shaped(&CLIFFS, seed(), x, y)).collect();
    assert!(forwards == backwards && forwards == columns && forwards == alone);
}

/// Superchunks made in one order, in another drawn by lot, and all at
/// once on threads of their own come to the same heights -- finer
/// meshes, cliffs and all.
#[test]
fn superchunks_made_in_any_order_are_the_same() {
    let shape = Shape { span: 11, sea: 0, narrow: 1 << 12, ..Shape::DEFAULT };
    let seed = seed();
    let about: Vec<SuperchunkIndex> = (0..9).map(|index| WORLD_MIDDLE.offset(index % 3 - 1, index / 3 - 1).expect("in the world")).collect();
    let made = |superchunk: SuperchunkIndex| Terrain::generate_shaped(&shape, seed, superchunk);
    let in_order: Vec<Terrain> = about.iter().map(|&superchunk| made(superchunk)).collect();
    // Another order, drawn by lot: each then put back where it is in the first.
    let mut order: Vec<usize> = (0..about.len()).collect();
    for last in (1..order.len()).rev() {
        order.swap(last, (utilities::hash::mix(last as u64 ^ 0x5EED) % (last as u64 + 1)) as usize);
    }
    assert_ne!(order, (0..about.len()).collect::<Vec<_>>(), "another order");
    let mut by_lot: Vec<Option<Terrain>> = (0..about.len()).map(|_| None).collect();
    for &which in &order {
        by_lot[which] = Some(made(about[which]));
    }
    let at_once: Vec<Terrain> = std::thread::scope(|scope| about.iter().map(|&superchunk| scope.spawn(move || made(superchunk))).collect::<Vec<_>>().into_iter().map(|made| made.join().expect("made")).collect());
    // The heights alone: the walls follow from them.
    let same = |one: &Terrain, other: &Terrain| one.heights == other.heights;
    assert!(in_order.iter().any(|terrain| terrain.wall_counts() != [0; 2]), "ground with something to it");
    for which in 0..about.len() {
        assert!(same(&in_order[which], by_lot[which].as_ref().expect("made")), "superchunk {which}, made by lot");
        assert!(same(&in_order[which], &at_once[which]), "superchunk {which}, made at once");
    }
}
