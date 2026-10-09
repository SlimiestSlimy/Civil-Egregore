//! The host: a world run on a thread of its own, asked for frames.

use server::host::frame::{Ask, Frame};
use server::host::Host;
use server::{Size, Start};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use utilities::tuning::{defaults, OCEAN_FLOOR};
use worldgen::Generation;

/// The first frame the host answers with that `wanted` takes, asking
/// again each second, for a minute at most.
fn frame_that(host: &Host, frames: &Receiver<Frame>, wanted: impl Fn(&Frame) -> bool) -> Frame {
    for _ in 0..60 {
        assert!(host.sync(Ask { viewport: None, detail: 0, skip: 0, most: 0, near: None }), "the host is there");
        if let Ok(frame) = frames.recv_timeout(Duration::from_secs(1))
            && wanted(&frame)
        {
            return frame;
        }
    }
    panic!("no such frame in a minute");
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
