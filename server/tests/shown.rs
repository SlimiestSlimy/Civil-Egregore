//! A world shown: a program of its own, since what has worlds shown is
//! the environment, which is the whole process's. One test, playing
//! the window: it asks a world someone else ticks for frames, pauses
//! it, and lets it go.
//!
//! `cargo test --test shown`

// What every test of a world shares.
#[path = "tests.rs"]
mod tests;

use server::host::frame::{FrameAsk, Viewport};
use server::host::shown::{SHOWN_WORLDS_PORT, RENDER_VARIABLE};
use server::host::Host;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long the test waits for anything that should come.
const WAIT_AT_MOST: Duration = Duration::from_secs(20);

/// A world ticked on a thread of its own with the environment asking
/// for worlds to be shown, the test in the window's place: a frame
/// asked for comes, of that world -- its seed, a tick it had reached,
/// its sheep, its superchunks' cells as the world holds them; paused,
/// it ticks no further until let go; and what it comes to is what the
/// same world comes to ticked unseen. Skipped, saying so, if a window
/// showing worlds is open on this machine: the port is its.
#[test]
fn a_world_ticked_is_shown_to_the_window_that_asks() {
    let Some((window, frame_receiver)) = Host::of_shown_worlds() else {
        eprintln!("skipped: port {SHOWN_WORLDS_PORT} is taken -- a window showing worlds is open");
        return;
    };
    // SAFETY: set before any thread of this program reads the environment -- the one test of the program, its ticking thread not yet started.
    unsafe { std::env::set_var(RENDER_VARIABLE, "1") };
    let mut test_random = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    let (seed, sheep_per_flock, ticks) = (tests::land_seed(0), test_random.draw_between(200, 2_000) as usize, test_random.draw_between(3_000, 6_000));
    let world_start = server::WorldStart { seed, sheep_per_flock, world_size: server::WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, ..server::WorldStart::default() };
    let tick_reached = Arc::new(AtomicU64::new(0));
    let ticking_thread = {
        let tick_reached_shared = Arc::clone(&tick_reached);
        std::thread::spawn(move || {
            let mut world = server::start_world(world_start);
            for _ in 0..ticks {
                world.tick();
                tick_reached_shared.store(world.entity_store().current_tick(), Ordering::SeqCst);
            }
            tests::world_contents(&world)
        })
    };
    let frame_ask = FrameAsk { viewport: Some(Viewport { top_left_superchunk_cartesian: (0, 0), bottom_right_superchunk_cartesian: (u32::MAX, u32::MAX) }), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None };
    assert!(window.ask_frame(frame_ask));
    let first_frame = frame_receiver.recv_timeout(WAIT_AT_MOST).expect("a frame of the world ticked");
    assert_eq!((first_frame.seed, first_frame.more_to_come, first_frame.viewport), (seed, false, frame_ask.viewport));
    assert!(first_frame.tick >= 1 && first_frame.tick <= ticks, "a tick the world had reached: {}", first_frame.tick);
    assert!(!first_frame.hot.is_empty() && first_frame.cells.len() == first_frame.hot.len(), "every hot superchunk of the viewport, copied: {} of {}", first_frame.cells.len(), first_frame.hot.len());
    assert!(first_frame.cells.iter().all(|cells| !cells.heights.is_empty() && !cells.grass_bitmaps.is_empty()), "a superchunk's first frame brings its heights");
    assert_eq!(first_frame.cells.iter().map(|cells| cells.sheep_cells_cartesian.len()).sum::<usize>(), first_frame.sheep_count, "the sheep drawn are the world's");
    // Paused, it ticks no further; a frame asked meanwhile is of the tick it stopped at.
    assert!(window.set_paused(true) && window.ask_frame(frame_ask));
    let paused_frame = frame_receiver.recv_timeout(WAIT_AT_MOST).expect("a frame of the world paused");
    std::thread::sleep(Duration::from_millis(300));
    let stopped_at = tick_reached.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    if stopped_at < ticks {
        assert_eq!(tick_reached.load(Ordering::SeqCst), stopped_at, "paused, it ticks no further");
        assert!(paused_frame.tick <= stopped_at + 1);
        assert!(paused_frame.cells.iter().all(|cells| cells.heights.is_empty()), "heights come the once");
    }
    assert!(window.set_paused(false));
    let waiting_began = Instant::now();
    while !ticking_thread.is_finished() && waiting_began.elapsed() < WAIT_AT_MOST {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ticking_thread.is_finished(), "let go, it ticks to its end");
    // Shown, it came to what it comes to unseen: this thread's world is ticked with the window gone.
    let shown_contents = ticking_thread.join().expect("ticked");
    drop((window, frame_receiver));
    let mut unseen_world = server::start_world(world_start);
    (0..ticks).for_each(|_| _ = unseen_world.tick());
    assert!(shown_contents == tests::world_contents(&unseen_world), "the world shown is the world unseen");
}
