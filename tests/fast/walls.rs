//! Walls of generated ground: no sheep steps through one, across, down
//! or diagonally.

use protocol::server_parameters::{CleanServerParameters, ParameterValue, SEED, SIDE};
use crate::probe::entities_of_the_turn;
use server::simulation::simulation_api::entity_reads_and_writes;
use utilities::coordinates::CellCartesian;

/// In a world generated, the ground has walls, and no sheep ever steps
/// through one: across or down only between cells at most a step apart
/// in height, diagonally only where both ways round are such steps.
#[test]
fn sheep_never_step_through_a_wall() {
    use std::collections::HashMap;
    // Small polygons joined by cliffs, and a seed whose origin superchunk has walls enough.
    let land_shape = basegame::worldgen::LandShape { vertex_spacing_power: 8, highest_land: 552, narrowest_blend: 2, widest_blend: 2, ocean_share: 0, finer_depth: 3, ..basegame::worldgen::LandShape::DEFAULT };
    let seed = (crate::test_seed::TestSeedHandle::open().seed()..).find(|&seed| basegame::worldgen::Terrain::generate_shaped(&land_shape, seed, utilities::coordinates::CellArea { top_left: utilities::coordinates::WORLD_MIDDLE_CELL, side_cells: utilities::coordinates::SUPERCHUNK_SIDE_CELLS }).wall_counts().iter().sum::<u64>() > 5_000).expect("a walled origin");
    let parameters = CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(1));
    let (mut made_world, probe) = crate::worlds::probed_world(&crate::worlds::in_folder(parameters, "sheep_never_step_through_a_wall", basegame::worldgen::WorldGeneration { land_shape, sheep_per_superchunk: 4_000, ..basegame::worldgen::WorldGeneration::DEFAULT }), crate::worlds::sheep_tag_registered);
    // The superchunk's heights and a cell more all round, worked out once: asked for at every sheep, every tick.
    let (corner_cartesian, side_cells) = (utilities::coordinates::WORLD_MIDDLE.top_left().cartesian(), utilities::coordinates::SUPERCHUNK_SIDE_CELLS as usize + 2);
    let mut mesh_lands = basegame::worldgen::mesh::WorldLand::new(&land_shape, seed);
    let heights: Vec<_> = (0..side_cells * side_cells).map(|cell_number| mesh_lands.height(corner_cartesian.x.wrapping_add((cell_number % side_cells) as u32).wrapping_sub(1), corner_cartesian.y.wrapping_add((cell_number / side_cells) as u32).wrapping_sub(1))).collect();
    let height_at = |asked_cell: utilities::coordinates::CellIndex| {
        let cell = asked_cell.cartesian();
        let (cells_across, cells_down) = (cell.x.wrapping_sub(corner_cartesian.x).wrapping_add(1) as usize, cell.y.wrapping_sub(corner_cartesian.y).wrapping_add(1) as usize);
        // A sheep strayed past the superchunk: the generator asked.
        if cells_across < side_cells && cells_down < side_cells { heights[cells_down * side_cells + cells_across] } else { basegame::worldgen::height_shaped(&land_shape, seed, cell.x, cell.y) }
    };
    // Each sheep told from the others by a tag -- the cell it stood on as it was given one -- and a lamb, born after, has none,
    // and is not followed. The flock made the first tick, by the world's generation; tagged in a whole tick, its rules run.
    made_world.tick();
    let sheep_tag = crate::worlds::sheep_tag(&made_world);
    let (_, tagged_cells) = probe.whole_tick(&mut made_world, move |tick_state| {
        let every_sheep = entities_of_the_turn(tick_state);
        every_sheep.iter().map(|tagged_sheep| {
            entity_reads_and_writes::set_attribute_of(tick_state, &tagged_sheep.header, sheep_tag, None, tagged_sheep.header.stood_cell.0);
            tagged_sheep.header.stood_cell
        }).collect()
    });
    let mut stood_cell_by_tag: HashMap<u64, utilities::coordinates::CellIndex> = tagged_cells.into_iter().map(|tagged_cell| (tagged_cell.0, tagged_cell)).collect();
    let (mut steps_moved, mut beside_walls) = (0, 0);
    for _ in 0..1_500 {
        // Every sheep as the whole tick found it: as the one before left it.
        let (_, seen_sheep) = probe.whole_tick(&mut made_world, move |tick_state| entities_of_the_turn(tick_state).iter().map(|seen_sheep| (crate::worlds::tag_of(sheep_tag, seen_sheep), seen_sheep.header.stood_cell)).collect());
        for (tag, stood_cell) in seen_sheep {
            if let Some(was_cell) = tag.and_then(|tag_number| stood_cell_by_tag.insert(tag_number, stood_cell)).filter(|&previous_cell| previous_cell != stood_cell) {
                let (step_from_cartesian, step_to_cartesian) = (was_cell.cartesian(), stood_cell.cartesian());
                // The cells of each way round: the straight step's alone, or the diagonal's two corners.
                let corner_cells = [CellCartesian { x: step_to_cartesian.x, y: step_from_cartesian.y }, CellCartesian { x: step_from_cartesian.x, y: step_to_cartesian.y }];
                let step_walled = corner_cells.iter().any(|corner_cell| basegame::worldgen::wall(height_at(was_cell), height_at((*corner_cell).into())) || basegame::worldgen::wall(height_at((*corner_cell).into()), height_at(stood_cell)));
                assert!(!step_walled, "from height {} to {}: {step_from_cartesian:?} to {step_to_cartesian:?}", height_at(was_cell), height_at(stood_cell));
                steps_moved += 1;
            }
            beside_walls += (0..9).any(|way| stood_cell.at_cartesian_offset(way % 3 - 1, way / 3 - 1).is_some_and(|beside_cell| basegame::worldgen::wall(height_at(stood_cell), height_at(beside_cell)))) as usize;
        }
    }
    // Few steps: on ground nearly all grass a sheep seldom has to walk.
    assert!(steps_moved > 50 && beside_walls > 10_000, "{steps_moved} steps, {beside_walls} sheep-ticks beside a wall");
}
