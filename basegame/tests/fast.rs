//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/terrain_cells.rs"]
mod terrain_cells;

mod terrain {
    //! Terrain: heights settled by the seed and the cell alone, seamless
    //! across superchunks, polygons joined by ramps; walls exactly where two cells beside
    //! one another are more than a step apart in height.
    //!
    //! `cargo test`

    use crate::terrain_cells::{area_beside, area_drawn, middle_area};
    use server::simulation::simulation_api::{CellArea, CellCartesian, RandomSource};
    use basegame::worldgen::{height_shaped, wall, LandShape, Terrain, DROP, STEP, WALLS};

    /// A shape of land and ocean little apart in height, joined by lines
    /// that slope their whole length, and nothing finer: no walls.
    const RAMPS_SHAPE: LandShape = LandShape { ocean_floor: 461, highest_land: 561, narrowest_blend: 1 << 16, widest_blend: 1 << 16, least_sigmoidness: 512, most_sigmoidness: 512, finer_depth: 0, ..LandShape::DEFAULT };
    /// A shape all land, of small triangles joined by cliffs: plenty of walls.
    const CLIFFS_SHAPE: LandShape = LandShape { vertex_spacing_power: 8, ocean_share: 0, highest_land: 552, narrowest_blend: 2, widest_blend: 2, finer_depth: 3, ..LandShape::DEFAULT };

    /// The same seed gives the same heights, another seed others; a
    /// superchunk's heights are the world's, whichever superchunk is made.
    #[test]
    fn heights_are_settled_by_the_seed_and_the_cell() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let mut seeded_rng = RandomSource::new(seed);
        let area = area_drawn(&mut seeded_rng);
        let CellCartesian { x: area_left, y: area_top } = area.top_left;
        let [first_terrain, again_terrain, other_terrain] = [seed, seed, seed + 1].map(|seed| Terrain::generate_shaped(&CLIFFS_SHAPE, seed, area));
        assert!(first_terrain.heights == again_terrain.heights);
        assert!(first_terrain.heights != other_terrain.heights);
        // Its corners, a chunk's border, and cells drawn.
        let drawn_cells: Vec<(u32, u32)> = (0..2_000).map(|_| (seeded_rng.draw_below(1024) as u32, seeded_rng.draw_below(1024) as u32)).collect();
        for (x, y) in [(0, 0), (1023, 1023), (1023, 0), (255, 256)].into_iter().chain(drawn_cells) {
            assert_eq!(first_terrain.height(x, y), height_shaped(&CLIFFS_SHAPE, seed, area_left + x, area_top + y));
        }
    }

    /// A wall is kept exactly where the cell stepped to is more than a
    /// step up or more than a drop down -- across the superchunk's edges
    /// too, by the heights beyond -- and with narrow edges some of the
    /// ground is walled, most of it not; and some drops are stepped
    /// down and not back up.
    #[test]
    fn walls_are_where_heights_are_more_than_a_step_apart() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let CellCartesian { x: area_left, y: area_top } = middle_area().top_left;
        let terrain = Terrain::generate_shaped(&CLIFFS_SHAPE, seed, middle_area());
        let height = |x, y| height_shaped(&CLIFFS_SHAPE, seed, x, y);
        for y in (0..1024).step_by(7).chain([1023]) {
            for x in (0..1024).step_by(5).chain([0, 1023]) {
                for (way, &(_, (dx, dy))) in WALLS.iter().enumerate() {
                    let (height_here, height_there) = (height(area_left + x, area_top + y), height((area_left + x).wrapping_add_signed(dx), (area_top + y).wrapping_add_signed(dy)));
                    let step_barred = height_there > height_here + STEP || height_here > height_there + DROP;
                    assert_eq!(terrain.keeps_wall(way, x, y), step_barred, "({x}, {y}) way {way}: {height_here} and {height_there}");
                    assert_eq!(wall(height_here, height_there), step_barred);
                }
            }
        }
        let counts = terrain.wall_counts();
        let walled_share = counts.iter().sum::<u64>() as f64 / (4.0 * 1024.0 * 1024.0);
        assert!(walled_share > 0.0005 && walled_share < 0.25, "{:.3}% of steps walled: {counts:?}", 100.0 * walled_share);
    }

    /// With broad edges the plains join by ramps and the shores fall
    /// gently: no wall, and no two cells beside one another more than a
    /// step apart.
    #[test]
    fn broad_edges_have_no_walls() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let terrain = Terrain::generate_shaped(&RAMPS_SHAPE, seed, middle_area());
        assert_eq!(terrain.wall_counts(), [0; 4]);
        let heights: Vec<u16> = (0..1024).map(|x| terrain.height(x, 500)).collect();
        assert!(heights.windows(2).all(|neighbour_pair| neighbour_pair[0].abs_diff(neighbour_pair[1]) <= 1), "a row: {heights:?}");
    }

    /// Flat ground has no walls; a cliff has them along it.
    #[test]
    fn a_cliff_is_walled_along_its_length() {
        assert_eq!(Terrain::from_heights(middle_area(), |_, _| 9).wall_counts(), [0; 4]);
        // Ground 3 higher from column 500 on: stepped down, not up.
        let terrain = Terrain::from_heights(middle_area(), |x, _| if x >= 500 { 3 } else { 0 });
        assert_eq!(terrain.wall_counts(), [1024, 0, 0, 0], "east walls down it, none back west: a drop to hop down");
        // Higher than a drop: walled both ways, the step west out of column 500 too.
        let high_terrain = Terrain::from_heights(middle_area(), |x, _| if x >= 500 { DROP + 1 } else { 0 });
        assert_eq!(high_terrain.wall_counts(), [1024, 0, 1024, 0]);
        assert!(high_terrain.keeps_wall(2, 500, 77) && !high_terrain.keeps_wall(2, 499, 77) && !high_terrain.keeps_wall(2, 501, 77));
        assert!(terrain.keeps_wall(0, 499, 77));
        assert!(!terrain.keeps_wall(0, 500, 77) && !terrain.keeps_wall(0, 498, 77));
    }

    /// The land as a mesh: some ocean at the lowest ground, some land over
    /// the ocean's level, and between any two cells beside one another --
    /// a line crossed or not -- a slope, never a break.
    #[test]
    fn the_mesh_is_ocean_or_land_joined_by_slopes() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let ramps_shape = LandShape { vertex_spacing_power: 10, ..RAMPS_SHAPE };
        let row_lands: Vec<u64> = (0..40_000).map(|x| basegame::worldgen::mesh::land_of_one_cell_cartesian(&ramps_shape, seed, 2_000_000_000 + x, 2_000_000_000)).collect();
        assert!(row_lands.iter().all(|&land_height| (ramps_shape.ocean_floor..=ramps_shape.highest_land.max(ramps_shape.ocean + 1)).contains(&(land_height as u16))));
        assert!(row_lands.contains(&(ramps_shape.ocean_floor as u64)) && row_lands.iter().any(|&land_height| land_height > ramps_shape.ocean as u64), "ocean and land both");
        assert!(row_lands.windows(2).all(|neighbour_pair| neighbour_pair[0].abs_diff(neighbour_pair[1]) <= 10), "no break");
    }

    /// A cell's height is the same in whatever order cells are asked for:
    /// one after another along a row, back along it, each alone, or down
    /// columns -- cliffs, finer meshes and all.
    #[test]
    fn heights_are_the_same_in_whatever_order_they_are_asked_for() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        use basegame::worldgen::mesh::WorldLand;
        let mut seeded_rng = RandomSource::new(seed);
        // A lattice of cells from a corner drawn, a few cells apart: over many of the mesh's lines.
        let (side_cells, cells_apart) = (300u32, seeded_rng.draw_between(1, 12) as u32);
        let (lattice_left, lattice_top) = (seeded_rng.draw_below((1 << 32) - u64::from(side_cells * cells_apart)) as u32, seeded_rng.draw_below((1 << 32) - u64::from(side_cells * cells_apart)) as u32);
        let cells = || (0..side_cells * side_cells).map(|cell| (lattice_left + cell % side_cells * cells_apart, lattice_top + cell / side_cells * cells_apart));
        let mut world_land = WorldLand::new(&CLIFFS_SHAPE, seed);
        let forwards_heights: Vec<u16> = cells().map(|(x, y)| world_land.height(x, y)).collect();
        let mut world_land = WorldLand::new(&CLIFFS_SHAPE, seed);
        let mut backwards_heights: Vec<u16> = cells().collect::<Vec<_>>().into_iter().rev().map(|(x, y)| world_land.height(x, y)).collect();
        backwards_heights.reverse();
        let mut world_land = WorldLand::new(&CLIFFS_SHAPE, seed);
        let mut column_heights = vec![0; forwards_heights.len()];
        for cell in 0..side_cells * side_cells {
            let (cell_across, cell_down) = (cell / side_cells, cell % side_cells);
            column_heights[(cell_down * side_cells + cell_across) as usize] = world_land.height(lattice_left + cell_across * cells_apart, lattice_top + cell_down * cells_apart);
        }
        let alone_heights: Vec<u16> = cells().map(|(x, y)| height_shaped(&CLIFFS_SHAPE, seed, x, y)).collect();
        assert!(forwards_heights == backwards_heights && forwards_heights == column_heights && forwards_heights == alone_heights);
    }

    /// Superchunks made in one order, in another drawn by lot, and all at
    /// once on threads of their own come to the same heights -- finer
    /// meshes, cliffs and all.
    #[test]
    fn superchunks_made_in_any_order_are_the_same() {
        let seed = crate::test_seed::TestSeedHandle::open().seed();
        let test_shape = LandShape { vertex_spacing_power: 11, ocean_share: 0, narrowest_blend: 1 << 12, ..LandShape::DEFAULT };
        let superchunks_about: Vec<CellArea> = (0..9).map(|about_index| area_beside(middle_area(), about_index % 3 - 1, about_index / 3 - 1)).collect();
        let make_terrain = |area: CellArea| Terrain::generate_shaped(&test_shape, seed, area);
        let in_order: Vec<Terrain> = superchunks_about.iter().map(|&superchunk| make_terrain(superchunk)).collect();
        // Another order, drawn by lot: each then put back where it is in the first.
        let mut lot_order: Vec<usize> = (0..superchunks_about.len()).collect();
        let mut seeded_rng = RandomSource::new(seed);
        while lot_order == (0..superchunks_about.len()).collect::<Vec<_>>() {
            for last_index in (1..lot_order.len()).rev() {
                lot_order.swap(last_index, seeded_rng.draw_below(last_index as u64 + 1) as usize);
            }
        }
        let mut by_lot: Vec<Option<Terrain>> = (0..superchunks_about.len()).map(|_| None).collect();
        for &which_index in &lot_order {
            by_lot[which_index] = Some(make_terrain(superchunks_about[which_index]));
        }
        let at_once: Vec<Terrain> = std::thread::scope(|thread_scope| superchunks_about.iter().map(|&superchunk| thread_scope.spawn(move || make_terrain(superchunk))).collect::<Vec<_>>().into_iter().map(|spawned_thread| spawned_thread.join().expect("made")).collect());
        // The heights alone: the walls follow from them.
        let same_heights = |one_terrain: &Terrain, other_terrain: &Terrain| one_terrain.heights == other_terrain.heights;
        assert!(in_order.iter().any(|terrain| terrain.wall_counts() != [0; 4]), "ground with something to it");
        for which_index in 0..superchunks_about.len() {
            assert!(same_heights(&in_order[which_index], by_lot[which_index].as_ref().expect("made")), "superchunk {which_index}, made by lot");
            assert!(same_heights(&in_order[which_index], &at_once[which_index]), "superchunk {which_index}, made at once");
        }
    }
}
