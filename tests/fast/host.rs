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
    let test_call = crate::worlds::test_type_named(&frozen_type_registry, crate::worlds::TEST_CALL).expect("registered").type_id();
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
