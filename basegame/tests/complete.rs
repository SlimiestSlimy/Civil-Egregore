//! The complete tier: many superchunks and seeds -- minutes at most.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --release --test complete -- --ignored`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/terrain_cells.rs"]
mod terrain_cells;

use server::simulation::simulation_api::RandomSource;
use terrain_cells::{area_beside, area_drawn, middle_area};
use basegame::worldgen::{wall, LandShape, Terrain};

/// A shape of small polygons joined by cliffs: plenty of walls.
const CLIFFS_SHAPE: LandShape = LandShape { vertex_spacing_power: 8, ocean_share: 0, highest_land: 552, narrowest_blend: 2, widest_blend: 2, finer_depth: 3, ..LandShape::DEFAULT };

/// Whatever the seed, with narrow edges little of the ground is walled.
#[test]
#[ignore]
fn walls_are_a_small_share_of_the_ground_whatever_the_seed() {
    let first_seed = crate::test_seed::TestSeedHandle::open().seed();
    for seed_offset in 1..=16 {
        let seed = first_seed + seed_offset;
        let counts = Terrain::generate_shaped(&CLIFFS_SHAPE, seed, area_beside(middle_area(), seed_offset as i32, 0)).wall_counts();
        let walled_share = counts.iter().sum::<u64>() as f64 / (4.0 * 1024.0 * 1024.0);
        assert!(walled_share < 0.25, "seed {seed}: {:.2}% of steps walled, {counts:?}", 100.0 * walled_share);
    }
}

/// Superchunks made apart meet with no seam: the walls one keeps along
/// its east and south edges are those its neighbours' heights make.
#[test]
#[ignore]
fn superchunks_made_apart_meet_with_no_seam() {
    let first_seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_rng = RandomSource::new(first_seed);
    for seed in [0, 1].map(|seed_offset| first_seed + seed_offset) {
        // Anywhere with a superchunk east and south of it.
        let here_area = area_drawn(&mut seeded_rng);
        let [own_terrain, east_terrain, south_terrain] = [(0, 0), (1, 0), (0, 1)].map(|(dx, dy)| Terrain::generate_shaped(&CLIFFS_SHAPE, seed, area_beside(here_area, dx, dy)));
        for along_edge in 0..1024 {
            assert_eq!(own_terrain.keeps_wall(0, 1023, along_edge), wall(own_terrain.height(1023, along_edge), east_terrain.height(0, along_edge)), "east edge, row {along_edge}");
            assert_eq!(own_terrain.keeps_wall(1, along_edge, 1023), wall(own_terrain.height(along_edge, 1023), south_terrain.height(along_edge, 0)), "south edge, column {along_edge}");
        }
    }
}
