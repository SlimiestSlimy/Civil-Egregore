//! The host: a world run on a thread of its own, asked for frames.

// TEMPORARY: reaches the server's internals -- `server::internals_for_root_tests`
// and the world's handles to them -- until the world has reads of its
// own and this is rewritten against them (AI.txt, LATER).

use protocol::server_parameters::{CleanServerParameters, ParameterValue, FORCED_HOT, SEED, SIDE, WORLDGEN};
use crate::worlds::base;
use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use server::host::{Host, HostSent};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use utilities::tuning::{default_tuning, OCEAN_FLOOR};
use basegame::worldgen::WorldGeneration;

/// The first frame the host answers with that `frame_wanted` takes. Each
/// asked after what was called before it, so the host answers as soon
/// as it has done that: waited for as long as a slow machine may take
/// to make a world, never counted in tries.
fn frame_that(host: &Host, frame_receiver: &Receiver<HostSent>, frame_wanted: impl Fn(&ViewportFrame) -> bool) -> ViewportFrame {
    loop {
        assert!(host.ask_frame(FrameAsk::default()), "the host is there");
        let answered_frame = next_frame(frame_receiver);
        if frame_wanted(&answered_frame) {
            return answered_frame;
        }
    }
}

/// The next frame the host sends, the types it sends once a world
/// passed by: waited for as long as a slow machine may take to make a
/// world.
fn next_frame(frame_receiver: &Receiver<HostSent>) -> ViewportFrame {
    loop {
        match frame_receiver.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world made") {
            HostSent::Frame(frame) => return *frame,
            HostSent::WorldRuns(..) => {}
        }
    }
}

/// A world reset with another generation: made again from its start
/// -- its seed and its sheep -- as another world, generated as asked.
#[test]
fn a_reset_makes_the_world_again_as_retuned() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let (host, frame_receiver) = Host::start_on_thread(basegame::default_types::register_types);
    // Paused, what a frame says is what the start made.
    assert!(host.set_paused(true));
    let world_start = CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(2)).with(WORLDGEN, crate::worlds::worldgen(crate::worlds::flocked(3)));
    assert!(host.make_world(world_start.clone(), crate::worlds::types()));
    let made_frame = frame_that(&host, &frame_receiver, |_| true);
    assert_eq!((made_frame.seed, made_frame.worldgen_parameters.clone(), made_frame.entity_count, made_frame.tick), (seed, crate::worlds::flocked(3), 0, 0), "the sheep made the first tick");
    let mut tuning = default_tuning();
    tuning[OCEAN_FLOOR] += 64.0;
    let retuned_generation = WorldGeneration::from_tuning(&tuning);
    assert_ne!(retuned_generation, WorldGeneration::DEFAULT);
    assert!(host.reset(&tuning));
    let reset = frame_that(&host, &frame_receiver, |answered_frame| answered_frame.worldgen_parameters == retuned_generation.worldgen_parameters());
    assert!(reset.world > made_frame.world, "another world");
    assert_eq!((reset.seed, reset.entity_count, reset.tick, reset.world_name), (seed, 0, 0, None));
}

/// The terrain a client asks of the host is the world's: every height
/// answered for a superchunk made is the one a frame brings of it, a
/// superchunk passed by is answered as 0, a map's cells have the
/// heights asked for one by one, none past the world's edges, and a
/// world run no more is answered with nothing.
#[test]
fn the_terrain_asked_for_is_what_the_frames_bring() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    use utilities::coordinates::{place_from_cartesian, SUPERCHUNK_SIDE_CELLS, WORLD_SIDE_SUPERCHUNKS};
        use protocol::pipe::terrain::{AsksTerrain, HeightsAsk, MapAsk};
    use protocol::pipe::frame::Viewport;
    let (host, frame_receiver) = Host::start_on_thread(basegame::default_types::register_types);
    let terrain = host.terrain();
    assert!(host.set_paused(true));
    assert!(host.make_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(2)).with(FORCED_HOT, crate::worlds::given()).with(WORLDGEN, crate::worlds::worldgen(crate::worlds::flocked(0))), crate::worlds::types()));
    let middle_coordinate = WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = Viewport { top_left_superchunk_cartesian: (middle_coordinate - 2, middle_coordinate - 2), bottom_right_superchunk_cartesian: (middle_coordinate + 2, middle_coordinate + 2) };
    let full_frame = loop {
        assert!(host.ask_frame(FrameAsk { viewport: Some(viewport), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None, planes: vec![base().height.layer_type()], counted: Vec::new() }), "the host is there");
        let answered_frame = next_frame(&frame_receiver);
        if answered_frame.cells.len() == 4 {
            break answered_frame;
        }
    };
    let side_cells = SUPERCHUNK_SIDE_CELLS;
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    for cells in &full_frame.cells {
        // Heights a plane of 16 bits a cell: four cells a word, its chunks one after another.
        let height_words = cells.plane(base().height.layer_type());
        assert!(!height_words.is_empty(), "heights come with every copy asking for them");
        let height_in = |place: usize| (height_words[place / 4] >> (16 * (place % 4))) as u16;
        let whole_ask = HeightsAsk { world: full_frame.world, first_cell_cartesian: cells.top_left, size_in_cells: (side_cells, side_cells), skipped_corners_cartesian: Vec::new() };
        let answered_heights = terrain.heights(whole_ask.clone()).expect("the world run");
        for (x, y) in (0..side_cells).flat_map(|y| (0..side_cells).map(move |x| (x, y))) {
            assert_eq!(answered_heights[(y * side_cells + x) as usize], height_in(place_from_cartesian(x, y)), "cell ({x}, {y}) of the superchunk at {:?}", cells.top_left);
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
    assert!(host.make_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(2)).with(FORCED_HOT, crate::worlds::given()).with(WORLDGEN, crate::worlds::worldgen(crate::worlds::flocked(0))), crate::worlds::types()));
    let remade_frame = frame_that(&host, &frame_receiver, |candidate_frame| candidate_frame.world > full_frame.world);
    assert!(terrain.map(MapAsk { world: full_frame.world, first_cell_cartesian: (0, 0), step: 1, size_in_pixels: (1, 1), mesh_lines_asked: false }).is_none(), "the world before, run no more");
    assert!(terrain.heights(HeightsAsk { world: remade_frame.world, first_cell_cartesian: (0, 0), size_in_cells: (1, 1), skipped_corners_cartesian: Vec::new() }).is_some());
}

/// A frame brings a plane of tiles as the world holds it: every
/// tile's number read off the frame by the tile's place, the tiles and
/// their numbers drawn from the seed; and nothing of a superchunk
/// none was written to.
#[test]
fn a_frame_brings_a_plane_of_tiles_a_tile_at_a_time() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    use server::internals_for_root_tests::Write;
    use utilities::coordinates::{place_from_cartesian, CellCartesian, SUPERCHUNK_SIDE_CELLS};
    let folder = crate::worlds::folder("tiles_in_frames");
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    let (frozen_type_registry, test_tiles) = crate::worlds::with_test_tiles();
    let tile_shift = test_tiles.layer_type().tile_shift();
    let mut world = server::start_world(&CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(2)).with(FORCED_HOT, crate::worlds::given()).with(WORLDGEN, crate::worlds::worldgen(crate::worlds::flocked(0))), &frozen_type_registry).expect("the world started");
    // Numbers on one of the four superchunks alone.
    let written_superchunk = world.bitmap_arena().superchunk_indices()[test_random.draw_below(4) as usize];
    let written_corner_cartesian = written_superchunk.top_left().cartesian();
    let mut numbers = std::collections::BTreeMap::new();
    for _ in 0..test_random.draw_between(50, 200) {
        let cell = (test_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, test_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32);
        // The tile's place among the superchunk's, in Morton order: its cell's, less the cells of a tile.
        let tile = place_from_cartesian(cell.0, cell.1) >> tile_shift;
        numbers.entry(tile).or_insert((cell, 1 + test_random.draw_below(u64::from(test_tiles.most_value())) as u32));
    }
    let writes: Vec<Write> = numbers.values().map(|&((x, y), number)| Write::value(test_tiles, CellCartesian { x: written_corner_cartesian.x + x, y: written_corner_cartesian.y + y }.into(), number)).collect();
    assert_eq!(world.write_cells(test_tiles.layer_type(), writes).cells_changed, numbers.len() as u64);
    server::save(&folder, &mut world).expect("saved");

    let (host, frame_receiver) = Host::start_on_thread(crate::worlds::types_with_test_tiles);
    assert!(host.set_paused(true) && host.open_world(folder.to_string_lossy().into_owned()));
    let middle_coordinate = utilities::coordinates::WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = protocol::pipe::frame::Viewport { top_left_superchunk_cartesian: (middle_coordinate - 2, middle_coordinate - 2), bottom_right_superchunk_cartesian: (middle_coordinate + 2, middle_coordinate + 2) };
    // Asked for a bounded number of times: a frame without the four superchunks is the world not opened yet.
    let four_superchunk_frame = (0..200).find_map(|_| {
        assert!(host.ask_frame(FrameAsk { viewport: Some(viewport), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None, planes: vec![test_tiles.layer_type()], counted: Vec::new() }), "the host is there");
        Some(next_frame(&frame_receiver)).filter(|answered_frame| answered_frame.cells.len() == 4)
    });
    for cells in &four_superchunk_frame.expect("a frame with the world's four superchunks").cells {
        if cells.top_left != (written_corner_cartesian.x, written_corner_cartesian.y) {
            assert!(cells.plane(test_tiles.layer_type()).is_empty(), "nothing sent of a superchunk with none");
            continue;
        }
        let tiles = (SUPERCHUNK_SIDE_CELLS as usize * SUPERCHUNK_SIDE_CELLS as usize) >> tile_shift;
        for tile in 0..tiles {
            assert_eq!(protocol::pipe::frame::value_at(cells.plane(test_tiles.layer_type()), test_tiles.layer_type().width_bits(), tile), numbers.get(&tile).map_or(0, |&(_, number)| number), "the tile {tile}");
        }
    }
}

/// A client's call is answered by the world running, between two ticks,
/// with what the function's code makes of its words -- here the tests'
/// own call, its world's seed and tick, then the words; and refused, the
/// host going on, when no world runs, when no type is of its ID, and
/// when the type of its ID is none a client may call.
#[test]
fn a_call_is_answered_by_the_world_running_and_only_a_client_call_is() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let frozen_type_registry = std::sync::Arc::new(protocol::world_types::of_a_new_world(crate::worlds::types_with_test_call).expect("the tests' types"));
    let test_call = frozen_type_registry.types().iter().find(|registered| registered.name() == crate::worlds::TEST_CALL).expect("registered").type_id();
    let (host, _host_sent) = Host::start_on_thread(crate::worlds::types_with_test_call);
    assert_eq!(host.call(test_call, vec![1]), Err("no world runs".to_string()));
    assert!(host.set_paused(true) && host.make_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), std::sync::Arc::clone(&frozen_type_registry)));
    let arguments = vec![seed ^ 7, 3, 5];
    assert_eq!(host.call(test_call, arguments.clone()), Ok([seed, 0].into_iter().chain(arguments).collect::<Vec<_>>()), "the seed, tick 0 -- paused -- and the words");
    let refused = |call: u64| host.call(call, Vec::new()).expect_err("refused");
    assert!(refused(test_call ^ 1 << 40).contains("no type is of the ID"));
    assert!(refused(base().grass.type_id()).contains("no function a client may call"), "a plane's ID");
    assert!(host.call(test_call, Vec::new()).is_ok(), "the host goes on after a call refused");
}

/// The world generation's calls: a worldgen parameter read and set for
/// the world's reset by its place, the world running generated as it
/// was until a call resets it -- made again then, another world of the
/// same seed, with the parameter set; a place past the parameters, or
/// words too few, refused.
#[test]
fn a_worldgen_parameter_set_by_a_call_holds_once_a_call_resets_the_world() {
    use basegame::worldgen::calls::{RESET_WORLD_CALL, SET_WORLDGEN_PARAMETER_CALL, WORLDGEN_PARAMETER_CALL};
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (host, frame_receiver) = Host::start_on_thread(basegame::default_types::register_types);
    assert!(host.set_paused(true) && host.make_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(WORLDGEN, crate::worlds::worldgen(crate::worlds::flocked(3))), crate::worlds::types()));
    let (read, set, reset) = (crate::worlds::type_id_named(WORLDGEN_PARAMETER_CALL), crate::worlds::type_id_named(SET_WORLDGEN_PARAMETER_CALL), crate::worlds::type_id_named(RESET_WORLD_CALL));
    let sheep_place = WorldGeneration::DEFAULT.worldgen_parameters().iter().position(|(name, _)| name == "sheep per superchunk").expect("a parameter") as u64;
    let sheep_of = |worldgen_parameters: &[(String, u64)]| worldgen_parameters.iter().find(|(name, _)| name == "sheep per superchunk").expect("a parameter").1;
    assert_eq!(host.call(read, vec![sheep_place]), Ok(vec![3]), "the world's own, before any is set");
    assert_eq!(host.call(set, vec![sheep_place, 5]), Ok(Vec::new()));
    assert_eq!(host.call(read, vec![sheep_place]), Ok(vec![5]), "set for the reset");
    let made_frame = frame_that(&host, &frame_receiver, |_| true);
    assert_eq!(sheep_of(&made_frame.worldgen_parameters), 3, "the world running generated as it was");
    assert!(host.call(read, vec![1 << 40]).expect_err("refused").contains("no worldgen parameter at"));
    assert!(host.call(set, vec![sheep_place]).is_err(), "a value is wanted");
    assert_eq!(host.call(reset, Vec::new()), Ok(Vec::new()));
    let reset_frame = frame_that(&host, &frame_receiver, |answered_frame| answered_frame.world > made_frame.world);
    assert_eq!((reset_frame.seed, sheep_of(&reset_frame.worldgen_parameters)), (seed, 5), "the same seed, the parameter set");
}
