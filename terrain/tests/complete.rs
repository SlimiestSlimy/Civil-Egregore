//! The complete tier: many superchunks and seeds -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release --test complete -- --ignored`

use coordinates::{place_from_cartesian, SuperchunkIndex, WORLD_MIDDLE};
use terrain::{wall, Shape, Terrain};

/// A shape of small polygons joined by cliffs: plenty of walls.
const CLIFFS: Shape = Shape { span: 8, highest: 552, edge: 2, ..Shape::DEFAULT };

/// The height of the cell `(x, y)` of a superchunk's `terrain`, from
/// its top left.
fn at(terrain: &Terrain, x: u32, y: u32) -> u16 {
    terrain.height(place_from_cartesian(x, y))
}

/// Whether `terrain` keeps a wall the `way`-th way at the cell `(x, y)`.
fn walled(terrain: &Terrain, way: usize, x: u32, y: u32) -> bool {
    terrain.walled(way, place_from_cartesian(x, y))
}

/// Whatever the seed, with narrow edges little of the ground is walled.
#[test]
#[ignore]
fn walls_are_a_small_share_of_the_ground_whatever_the_seed() {
    for seed in 1..=16 {
        let counts = Terrain::generate_shaped(&CLIFFS, seed, WORLD_MIDDLE.offset(seed as i32, 0).expect("in the world")).wall_counts();
        let share = counts.iter().sum::<u64>() as f64 / (2.0 * 1024.0 * 1024.0);
        assert!(share < 0.08, "seed {seed}: {:.2}% of steps walled, {counts:?}", 100.0 * share);
    }
}

/// Superchunks made apart meet with no seam: the walls one keeps along
/// its east and south edges are those its neighbours' heights make.
#[test]
#[ignore]
fn superchunks_made_apart_meet_with_no_seam() {
    for seed in [3, 4] {
        let here = SuperchunkIndex::from_cartesian(2_097_100, 2_097_200);
        let [own, east, south] = [(0, 0), (1, 0), (0, 1)].map(|(dx, dy)| Terrain::generate_shaped(&CLIFFS, seed, here.offset(dx, dy).expect("in the world")));
        for along in 0..1024 {
            assert_eq!(walled(&own, 0, 1023, along), wall(at(&own, 1023, along), at(&east, 0, along)), "east edge, row {along}");
            assert_eq!(walled(&own, 1, along, 1023), wall(at(&own, along, 1023), at(&south, along, 0)), "south edge, column {along}");
        }
    }
}
