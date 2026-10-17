//! The host: a world run on a thread of its own, asked for frames.

use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use server::host::Host;
use server::{WorldSize, WorldStart};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use utilities::tuning::{default_tuning, OCEAN_FLOOR};
use worldgen::WorldGeneration;

/// The first frame the host answers with that `frame_wanted` takes. Each
/// asked after what was called before it, so the host answers as soon
/// as it has done that: waited for as long as a slow machine may take
/// to make a world, never counted in tries.
fn frame_that(host: &Host, frame_receiver: &Receiver<ViewportFrame>, frame_wanted: impl Fn(&ViewportFrame) -> bool) -> ViewportFrame {
    loop {
        assert!(host.ask_frame(FrameAsk { viewport: None, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 0, seen_near: None }), "the host is there");
        let answered_frame = frame_receiver.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world made");
        if frame_wanted(&answered_frame) {
            return answered_frame;
        }
    }
}

/// A world reset with another generation: made again from its start
/// -- its seed and its sheep -- as another world, generated as asked.
#[test]
fn a_reset_makes_the_world_again_as_retuned() {
    let seed = server::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT);
    let (host, frame_receiver) = Host::start_on_thread();
    // Paused, what a frame says is what the start made.
    assert!(host.set_paused(true));
    let world_start = WorldStart { seed, world_size: WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: false }, world_generation: crate::worlds::flocked(3), ..WorldStart::default() };
    assert!(host.make_world(world_start));
    let made_frame = frame_that(&host, &frame_receiver, |_| true);
    assert_eq!((made_frame.seed, made_frame.world_generation, made_frame.sheep_count, made_frame.tick), (world_start.seed, crate::worlds::flocked(3), 0, 0), "the sheep made the first tick");
    let mut tuning = default_tuning();
    tuning[OCEAN_FLOOR] += 64.0;
    let retuned_generation = WorldGeneration::from_tuning(&tuning);
    assert_ne!(retuned_generation, WorldGeneration::DEFAULT);
    assert!(host.reset(&tuning));
    let reset = frame_that(&host, &frame_receiver, |answered_frame| answered_frame.world_generation == retuned_generation);
    assert!(reset.world > made_frame.world, "another world");
    assert_eq!((reset.seed, reset.sheep_count, reset.tick, reset.world_name), (world_start.seed, 0, 0, None));
}

/// The terrain a client asks of the host is the world's: every height
/// answered for a superchunk made is the one a frame brings of it, a
/// superchunk passed by is answered as 0, a map's cells have the
/// heights asked for one by one, none past the world's edges, and a
/// world run no more is answered with nothing.
#[test]
fn the_terrain_asked_for_is_what_the_frames_bring() {
    let seed = server::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT);
    use coordinates::{place_from_cartesian, SUPERCHUNK_SIDE_CELLS, WORLD_SIDE_SUPERCHUNKS};
    use protocol::pipe::frame::height_in_frame;
    use protocol::pipe::terrain::{AsksTerrain, HeightsAsk, MapAsk};
    use simulation::halos::Viewport;
    let (host, frame_receiver) = Host::start_on_thread();
    let terrain = host.terrain();
    assert!(host.set_paused(true));
    assert!(host.make_world(WorldStart { seed, world_size: WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, world_generation: crate::worlds::flocked(0), ..WorldStart::default() }));
    let middle_coordinate = WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = Viewport { top_left_superchunk_cartesian: (middle_coordinate - 2, middle_coordinate - 2), bottom_right_superchunk_cartesian: (middle_coordinate + 2, middle_coordinate + 2) };
    let full_frame = loop {
        assert!(host.ask_frame(FrameAsk { viewport: Some(viewport), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None }), "the host is there");
        let answered_frame = frame_receiver.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world made");
        if answered_frame.cells.len() == 4 {
            break answered_frame;
        }
    };
    let side_cells = SUPERCHUNK_SIDE_CELLS;
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    for cells in &full_frame.cells {
        assert!(!cells.heights.is_empty(), "heights come the first time a superchunk is copied");
        let whole_ask = HeightsAsk { world: full_frame.world, first_cell_cartesian: cells.top_left, size_in_cells: (side_cells, side_cells), skipped_corners_cartesian: Vec::new() };
        let answered_heights = terrain.heights(whole_ask.clone()).expect("the world run");
        for (x, y) in (0..side_cells).flat_map(|y| (0..side_cells).map(move |x| (x, y))) {
            assert_eq!(answered_heights[(y * side_cells + x) as usize], height_in_frame(&cells.heights, place_from_cartesian(x, y)), "cell ({x}, {y}) of the superchunk at {:?}", cells.top_left);
        }
        // A rectangle drawn anywhere over the superchunk and past it: the cells of the whole, and none of a superchunk passed by.
        let (first_offset, rect_size) = ((test_random.draw_below(u64::from(side_cells)) as u32, test_random.draw_below(u64::from(side_cells)) as u32), (test_random.draw_between(1, 300) as u32, test_random.draw_between(1, 300) as u32));
        let within_ask = HeightsAsk { first_cell_cartesian: (cells.top_left.0 + first_offset.0, cells.top_left.1 + first_offset.1), size_in_cells: rect_size, ..whole_ask.clone() };
        let (part, passed_heights) = (terrain.heights(within_ask.clone()).expect("the world run"), terrain.heights(HeightsAsk { skipped_corners_cartesian: vec![cells.top_left], ..within_ask.clone() }).expect("the world run"));
        let map = terrain.map(MapAsk { world: full_frame.world, first_cell_cartesian: (i64::from(within_ask.first_cell_cartesian.0), i64::from(within_ask.first_cell_cartesian.1)), step: 1, size_in_pixels: rect_size, mesh_lines_asked: false }).expect("the world run");
        assert_eq!(map.drawn_heights, full_frame.drawn_heights);
        for (x, y) in (0..rect_size.1).flat_map(|y| (0..rect_size.0).map(move |x| (x, y))) {
            let (row_major_index, inside_superchunk) = ((y * rect_size.0 + x) as usize, first_offset.0 + x < side_cells && first_offset.1 + y < side_cells);
            if inside_superchunk {
                assert_eq!(part[row_major_index], answered_heights[((first_offset.1 + y) * side_cells + first_offset.0 + x) as usize]);
                assert_eq!(passed_heights[row_major_index], 0, "a cell of a superchunk passed by");
            } else {
                assert_eq!(passed_heights[row_major_index], part[row_major_index], "a cell of another superchunk, not passed by");
            }
            assert_eq!(map.cells[row_major_index].expect("a cell of the world").height, part[row_major_index]);
        }
        assert_eq!(terrain.heights(HeightsAsk { world: full_frame.world + 1, ..within_ask }), None, "a world not run");
    }
    let corner_map = terrain.map(MapAsk { world: full_frame.world, first_cell_cartesian: (-1, -1), step: 1, size_in_pixels: (2, 2), mesh_lines_asked: false }).expect("the world run");
    assert_eq!(corner_map.cells.iter().map(Option::is_some).collect::<Vec<_>>(), [false, false, false, true], "only the world's first cell is in it");
    assert!(host.make_world(WorldStart { seed, world_size: WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, world_generation: crate::worlds::flocked(0), ..WorldStart::default() }));
    let next_frame = frame_that(&host, &frame_receiver, |candidate_frame| candidate_frame.world > full_frame.world);
    assert!(terrain.map(MapAsk { world: full_frame.world, first_cell_cartesian: (0, 0), step: 1, size_in_pixels: (1, 1), mesh_lines_asked: false }).is_none(), "the world before, run no more");
    assert!(terrain.heights(HeightsAsk { world: next_frame.world, first_cell_cartesian: (0, 0), size_in_cells: (1, 1), skipped_corners_cartesian: Vec::new() }).is_some());
}

/// A frame brings the clouds over each superchunk as the world holds
/// them: every tile's cloud read off the frame by the tile's place,
/// the tiles and their clouds drawn from the seed; and none where a
/// superchunk has no cloud.
#[test]
fn a_frame_brings_the_clouds_a_tile_at_a_time() {
    let seed = server::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &server::WorldGeneration::DEFAULT);
    use bitplane_manager::Write;
    use coordinates::{place_from_cartesian, CellCartesian, SUPERCHUNK_SIDE_CELLS};
    use protocol::pipe::frame::{cloud_over, CLOUD_TILE_SHIFT};
    use base_types::CLOUD;
    let folder = crate::worlds::folder("clouds");
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    let mut world = server::start_world(WorldStart { seed, world_size: WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, world_generation: crate::worlds::flocked(0), ..WorldStart::default() });
    // Clouds over one of the four superchunks alone.
    let clouded_superchunk = world.bitmap_arena().superchunk_indices()[test_random.draw_below(4) as usize];
    let clouded_corner_cartesian = clouded_superchunk.top_left().cartesian();
    let mut clouds = std::collections::BTreeMap::new();
    for _ in 0..test_random.draw_between(50, 200) {
        let cell = (test_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, test_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32);
        // The tile's place among the superchunk's, in Morton order: its cell's, less the cells of a tile.
        let tile = place_from_cartesian(cell.0, cell.1) >> CLOUD_TILE_SHIFT;
        clouds.entry(tile).or_insert((cell, 1 + test_random.draw_below(u64::from(CLOUD.most_value())) as u32));
    }
    let writes: Vec<Write> = clouds.values().map(|&((x, y), cloud)| Write::value(CLOUD, CellCartesian { x: clouded_corner_cartesian.x + x, y: clouded_corner_cartesian.y + y }.into(), cloud)).collect();
    assert_eq!(world.write_cells(CLOUD.layer_type(), writes).cells_changed, clouds.len() as u64);
    server::save(&folder, &mut world).expect("saved");

    let (host, frame_receiver) = Host::start_on_thread();
    assert!(host.set_paused(true) && host.open_world(folder.to_string_lossy().into_owned()));
    let middle_coordinate = coordinates::WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = simulation::halos::Viewport { top_left_superchunk_cartesian: (middle_coordinate - 2, middle_coordinate - 2), bottom_right_superchunk_cartesian: (middle_coordinate + 2, middle_coordinate + 2) };
    // Asked for a bounded number of times: a frame without the four superchunks is the world not opened yet.
    let four_superchunk_frame = (0..200).find_map(|_| {
        assert!(host.ask_frame(FrameAsk { viewport: Some(viewport), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None }), "the host is there");
        Some(frame_receiver.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world opened")).filter(|answered_frame| answered_frame.cells.len() == 4)
    });
    for cells in &four_superchunk_frame.expect("a frame with the world's four superchunks").cells {
        if cells.top_left != (clouded_corner_cartesian.x, clouded_corner_cartesian.y) {
            assert!(cells.clouds.is_empty(), "no cloud sent of a superchunk with none");
            continue;
        }
        let tiles = (SUPERCHUNK_SIDE_CELLS as usize * SUPERCHUNK_SIDE_CELLS as usize) >> CLOUD_TILE_SHIFT;
        for tile in 0..tiles {
            assert_eq!(cloud_over(&cells.clouds, tile), clouds.get(&tile).map_or(0, |&(_, cloud)| cloud), "the tile {tile}");
        }
    }
}
