//! The complete tier: many superchunks and seeds -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release --test complete -- --ignored`

mod tests;

use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
use tests::{height_at, keeps_wall_cartesian};
use worldgen::{wall, LandShape, Terrain};

/// A shape of small polygons joined by cliffs: plenty of walls.
const CLIFFS_SHAPE: LandShape = LandShape { vertex_spacing_power: 8, ocean_share: 0, highest_land: 552, narrowest_blend: 2, widest_blend: 2, finer_depth: 3, ..LandShape::DEFAULT };

/// Whatever the seed, with narrow edges little of the ground is walled.
#[test]
#[ignore]
fn walls_are_a_small_share_of_the_ground_whatever_the_seed() {
    for seed_offset in 1..=16 {
        let seed = utilities::seed::counted_seed() + seed_offset;
        let counts = Terrain::generate_shaped(&CLIFFS_SHAPE, seed, WORLD_MIDDLE.at_cartesian_offset(seed_offset as i32, 0).expect("in the world")).wall_counts();
        let walled_share = counts.iter().sum::<u64>() as f64 / (4.0 * 1024.0 * 1024.0);
        assert!(walled_share < 0.25, "seed {seed}: {:.2}% of steps walled, {counts:?}", 100.0 * walled_share);
    }
}

/// Superchunks made apart meet with no seam: the walls one keeps along
/// its east and south edges are those its neighbours' heights make.
#[test]
#[ignore]
fn superchunks_made_apart_meet_with_no_seam() {
    let mut seeded_rng = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    for seed in [0, 1].map(|seed_offset| utilities::seed::counted_seed() + seed_offset) {
        // Anywhere with a superchunk east and south of it.
        let last_drawable = u64::from(coordinates::WORLD_SIDE_SUPERCHUNKS) - 1;
        let here_superchunk = SuperchunkIndex::from_cartesian(seeded_rng.draw_below(last_drawable) as u32, seeded_rng.draw_below(last_drawable) as u32);
        let [own_terrain, east_terrain, south_terrain] = [(0, 0), (1, 0), (0, 1)].map(|(dx, dy)| Terrain::generate_shaped(&CLIFFS_SHAPE, seed, here_superchunk.at_cartesian_offset(dx, dy).expect("in the world")));
        for along_edge in 0..1024 {
            assert_eq!(keeps_wall_cartesian(&own_terrain, 0, 1023, along_edge), wall(height_at(&own_terrain, 1023, along_edge), height_at(&east_terrain, 0, along_edge)), "east edge, row {along_edge}");
            assert_eq!(keeps_wall_cartesian(&own_terrain, 1, along_edge, 1023), wall(height_at(&own_terrain, along_edge, 1023), height_at(&south_terrain, along_edge, 0)), "south edge, column {along_edge}");
        }
    }
}
