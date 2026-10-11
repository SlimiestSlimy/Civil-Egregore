//! The host: a world run on a thread of its own, asked for frames.

use server::host::frame::{Ask, Frame};
use server::host::Host;
use server::{Size, Start};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use utilities::tuning::{defaults, OCEAN_FLOOR};
use worldgen::Generation;

/// The first frame the host answers with that `wanted` takes. Each
/// asked after what was called before it, so the host answers as soon
/// as it has done that: waited for as long as a slow machine may take
/// to make a world, never counted in tries.
fn frame_that(host: &Host, frames: &Receiver<Frame>, wanted: impl Fn(&Frame) -> bool) -> Frame {
    loop {
        assert!(host.sync(Ask { viewport: None, detail: 0, skip: 0, most: 0, near: None }), "the host is there");
        let frame = frames.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world made");
        if wanted(&frame) {
            return frame;
        }
    }
}

/// A world reset with another generation: made again from its start
/// -- its seed and its sheep -- as another world, generated as asked.
#[test]
fn a_reset_makes_the_world_again_as_retuned() {
    let (host, frames) = Host::start();
    // Paused, what a frame says is what the start made.
    assert!(host.pause(true));
    let start = Start { seed: crate::tests::land_seed(3), size: Size::Limited { side: 2, forced: false }, sheep: 3, ..Start::default() };
    assert!(host.make_world(start));
    let made = frame_that(&host, &frames, |_| true);
    assert_eq!((made.seed, made.generation, made.sheep, made.tick), (start.seed, Generation::DEFAULT, 4 * 3, 0));
    let mut tuning = defaults();
    tuning[OCEAN_FLOOR] += 64.0;
    let retuned = Generation::from_tuning(&tuning);
    assert_ne!(retuned, Generation::DEFAULT);
    assert!(host.reset(&tuning));
    let reset = frame_that(&host, &frames, |frame| frame.generation == retuned);
    assert!(reset.world > made.world, "another world");
    assert_eq!((reset.seed, reset.sheep, reset.tick, reset.named), (start.seed, 4 * 3, 0, None));
}

/// The terrain a client asks of the host is the world's: every height
/// answered for a superchunk made is the one a frame brings of it, a
/// superchunk passed by is answered as 0, a map's cells have the
/// heights asked for one by one, none past the world's edges, and a
/// world run no more is answered with nothing.
#[test]
fn the_terrain_asked_for_is_what_the_frames_bring() {
    use coordinates::{place_from_cartesian, SUPERCHUNK_SIDE_CELLS, WORLD_SIDE_SUPERCHUNKS};
    use server::host::frame::height_in_frame;
    use server::host::terrain::{HeightsAsk, MapAsk};
    use simulation::halos::Viewport;
    let (host, frames) = Host::start();
    let terrain = host.terrain();
    assert!(host.pause(true));
    assert!(host.make_world(Start { seed: crate::tests::land_seed(5), size: Size::Limited { side: 2, forced: true }, sheep: 0, ..Start::default() }));
    let middle = WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = Viewport { first: (middle - 2, middle - 2), last: (middle + 2, middle + 2) };
    let frame = loop {
        assert!(host.sync(Ask { viewport: Some(viewport), detail: 0, skip: 0, most: u32::MAX, near: None }), "the host is there");
        let frame = frames.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world made");
        if frame.cells.len() == 4 {
            break frame;
        }
    };
    let side = SUPERCHUNK_SIDE_CELLS;
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    for cells in &frame.cells {
        assert!(!cells.heights.is_empty(), "heights come the first time a superchunk is copied");
        let whole = HeightsAsk { world: frame.world, first: cells.top_left, size: (side, side), skipped: Vec::new() };
        let answered = terrain.heights(whole.clone()).expect("the world run");
        for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
            assert_eq!(answered[(y * side + x) as usize], height_in_frame(&cells.heights, place_from_cartesian(x, y)), "cell ({x}, {y}) of the superchunk at {:?}", cells.top_left);
        }
        // A rectangle drawn anywhere over the superchunk and past it: the cells of the whole, and none of a superchunk passed by.
        let (first, size) = ((random.below(u64::from(side)) as u32, random.below(u64::from(side)) as u32), (random.between(1, 300) as u32, random.between(1, 300) as u32));
        let within = HeightsAsk { first: (cells.top_left.0 + first.0, cells.top_left.1 + first.1), size, ..whole.clone() };
        let (part, passed) = (terrain.heights(within.clone()).expect("the world run"), terrain.heights(HeightsAsk { skipped: vec![cells.top_left], ..within.clone() }).expect("the world run"));
        let map = terrain.map(MapAsk { world: frame.world, first: (i64::from(within.first.0), i64::from(within.first.1)), step: 1, size, borders: false }).expect("the world run");
        assert_eq!(map.levels, frame.levels);
        for (x, y) in (0..size.1).flat_map(|y| (0..size.0).map(move |x| (x, y))) {
            let (at, inside) = ((y * size.0 + x) as usize, first.0 + x < side && first.1 + y < side);
            if inside {
                assert_eq!(part[at], answered[((first.1 + y) * side + first.0 + x) as usize]);
                assert_eq!(passed[at], 0, "a cell of a superchunk passed by");
            } else {
                assert_eq!(passed[at], part[at], "a cell of another superchunk, not passed by");
            }
            assert_eq!(map.cells[at].expect("a cell of the world").height, part[at]);
        }
        assert_eq!(terrain.heights(HeightsAsk { world: frame.world + 1, ..within }), None, "a world not run");
    }
    let corner = terrain.map(MapAsk { world: frame.world, first: (-1, -1), step: 1, size: (2, 2), borders: false }).expect("the world run");
    assert_eq!(corner.cells.iter().map(Option::is_some).collect::<Vec<_>>(), [false, false, false, true], "only the world's first cell is in it");
    assert!(host.make_world(Start { seed: crate::tests::land_seed(6), size: Size::Limited { side: 2, forced: true }, sheep: 0, ..Start::default() }));
    let next = frame_that(&host, &frames, |next| next.world > frame.world);
    assert!(terrain.map(MapAsk { world: frame.world, first: (0, 0), step: 1, size: (1, 1), borders: false }).is_none(), "the world before, run no more");
    assert!(terrain.heights(HeightsAsk { world: next.world, first: (0, 0), size: (1, 1), skipped: Vec::new() }).is_some());
}

/// A frame brings the clouds over each superchunk as the world holds
/// them: every tile's cloud read off the frame by the tile's place,
/// the tiles and their clouds drawn from the seed; and none where a
/// superchunk has no cloud.
#[test]
fn a_frame_brings_the_clouds_a_tile_at_a_time() {
    use bitplane_manager::Write;
    use coordinates::{place_from_cartesian, CellCartesian, SUPERCHUNK_SIDE_CELLS};
    use server::host::frame::{cloud_over, CLOUD_TILE_SHIFT};
    use type_registry::CLOUD;
    let folder = crate::tests::folder("clouds");
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let mut world = server::start(Start { seed: crate::tests::land_seed(7), size: Size::Limited { side: 2, forced: true }, sheep: 0, ..Start::default() });
    // Clouds over one of the four superchunks alone.
    let clouded = world.arena().superchunk_indices()[random.below(4) as usize];
    let corner = clouded.top_left().cartesian();
    let mut clouds = std::collections::BTreeMap::new();
    for _ in 0..random.between(50, 200) {
        let cell = (random.below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, random.below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32);
        // The tile's place among the superchunk's, in Morton order: its cell's, less the cells of a tile.
        let tile = place_from_cartesian(cell.0, cell.1) >> CLOUD_TILE_SHIFT;
        clouds.entry(tile).or_insert((cell, 1 + random.below(u64::from(CLOUD.most())) as u32));
    }
    let writes: Vec<Write> = clouds.values().map(|&((x, y), cloud)| Write::value(CLOUD, CellCartesian { x: corner.x + x, y: corner.y + y }.into(), cloud)).collect();
    assert_eq!(world.write_cells(CLOUD.layer_type(), writes).changed, clouds.len() as u64);
    server::save(&folder, &mut world).expect("saved");

    let (host, frames) = Host::start();
    assert!(host.pause(true) && host.open_world(folder.to_string_lossy().into_owned()));
    let middle = coordinates::WORLD_SIDE_SUPERCHUNKS / 2;
    let viewport = simulation::halos::Viewport { first: (middle - 2, middle - 2), last: (middle + 2, middle + 2) };
    // Asked for a bounded number of times: a frame without the four superchunks is the world not opened yet.
    let frame = (0..200).find_map(|_| {
        assert!(host.sync(Ask { viewport: Some(viewport), detail: 0, skip: 0, most: u32::MAX, near: None }), "the host is there");
        Some(frames.recv_timeout(Duration::from_secs(1800)).expect("a frame of the world opened")).filter(|frame| frame.cells.len() == 4)
    });
    for cells in &frame.expect("a frame with the world's four superchunks").cells {
        if cells.top_left != (corner.x, corner.y) {
            assert!(cells.clouds.is_empty(), "no cloud sent of a superchunk with none");
            continue;
        }
        let tiles = (SUPERCHUNK_SIDE_CELLS as usize * SUPERCHUNK_SIDE_CELLS as usize) >> CLOUD_TILE_SHIFT;
        for tile in 0..tiles {
            assert_eq!(cloud_over(&cells.clouds, tile), clouds.get(&tile).map_or(0, |&(_, cloud)| cloud), "the tile {tile}");
        }
    }
}
