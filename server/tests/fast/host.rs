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
