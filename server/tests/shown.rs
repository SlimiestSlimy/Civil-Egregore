//! A world shown: a program of its own, since what has worlds shown is
//! the environment, which is the whole process's. One test, playing
//! the window: it asks a world someone else ticks for frames, pauses
//! it, and lets it go.
//!
//! `cargo test --test shown`

// What every test of a world shares.
#[path = "tests.rs"]
mod tests;

use server::host::frame::{Ask, Viewport};
use server::host::shown::{PORT, RENDER};
use server::host::Host;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the test waits for anything that should come.
const SOON: Duration = Duration::from_secs(20);

/// A world ticked on a thread of its own with the environment asking
/// for worlds to be shown, the test in the window's place: a frame
/// asked for comes, of that world -- its seed, a tick it had reached,
/// its sheep, its superchunks' cells as the world holds them; paused,
/// it ticks no further until let go; and what it comes to is what the
/// same world comes to ticked unseen. Skipped, saying so, if a window
/// showing worlds is open on this machine: the port is its.
#[test]
fn a_world_ticked_is_shown_to_the_window_that_asks() {
    let Some((window, frames)) = Host::of_shown_worlds() else {
        eprintln!("skipped: port {PORT} is taken -- a window showing worlds is open");
        return;
    };
    // SAFETY: set before any thread of this program reads the environment -- the one test of the program, its ticking thread not yet started.
    unsafe { std::env::set_var(RENDER, "1") };
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let (seed, sheep, ticks) = (tests::land_seed(0), random.between(200, 2_000) as usize, random.between(3_000, 6_000));
    let start = server::Start { seed, sheep, size: server::Size::Limited { side: 2, forced: true }, ..server::Start::default() };
    let reached = Arc::new(AtomicU64::new(0));
    let ticking = {
        let reached = Arc::clone(&reached);
        std::thread::spawn(move || {
            let mut world = server::start(start);
            for _ in 0..ticks {
                world.tick();
                reached.store(world.entities().now(), Ordering::SeqCst);
            }
            tests::everything(&world)
        })
    };
    let ask = Ask { viewport: Some(Viewport { first: (0, 0), last: (u32::MAX, u32::MAX) }), detail: 0, skip: 0, most: u32::MAX, near: None };
    assert!(window.sync(ask));
    let frame = frames.recv_timeout(SOON).expect("a frame of the world ticked");
    assert_eq!((frame.seed, frame.more, frame.viewport), (seed, false, ask.viewport));
    assert!(frame.tick >= 1 && frame.tick <= ticks, "a tick the world had reached: {}", frame.tick);
    assert!(!frame.hot.is_empty() && frame.cells.len() == frame.hot.len(), "every hot superchunk of the viewport, copied: {} of {}", frame.cells.len(), frame.hot.len());
    assert!(frame.cells.iter().all(|cells| !cells.heights.is_empty() && !cells.grass.is_empty()), "a superchunk's first frame brings its heights");
    assert_eq!(frame.cells.iter().map(|cells| cells.sheep.len()).sum::<usize>(), frame.sheep, "the sheep drawn are the world's");
    // Paused, it ticks no further; a frame asked meanwhile is of the tick it stopped at.
    assert!(window.pause(true) && window.sync(ask));
    let paused = frames.recv_timeout(SOON).expect("a frame of the world paused");
    std::thread::sleep(Duration::from_millis(300));
    let stopped_at = reached.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    if stopped_at < ticks {
        assert_eq!(reached.load(Ordering::SeqCst), stopped_at, "paused, it ticks no further");
        assert!(paused.tick <= stopped_at + 1);
        assert!(paused.cells.iter().all(|cells| cells.heights.is_empty()), "heights come the once");
    }
    assert!(window.pause(false));
    let began = Instant::now();
    while !ticking.is_finished() && began.elapsed() < SOON {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ticking.is_finished(), "let go, it ticks to its end");
    // Shown, it came to what it comes to unseen: this thread's world is ticked with the window gone.
    let shown = ticking.join().expect("ticked");
    drop((window, frames));
    let mut unseen = server::start(start);
    (0..ticks).for_each(|_| _ = unseen.tick());
    assert!(shown == tests::everything(&unseen), "the world shown is the world unseen");
}
