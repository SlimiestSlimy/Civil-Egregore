//! What answering a client costs the simulation: a world whose camera
//! loads, a wide viewport kept hot, and its ticks a second while no
//! frame is asked for and while frames are asked for as a window asks
//! -- one at a time, sixty a second at most, going round the viewport.

use crate::asking_the_host::{frame_that, seed, whole_answer};
use coordinates::WORLD_MIDDLE;
use server::host::frame::{Ask, Frame, Viewport};
use server::host::Host;
use server::Start;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use utilities::commands::Given;
use utilities::tuning::{defaults, CAMERA_LOADS, SHEEP, WORLD_SIDE};

/// Sheep each superchunk generated in the viewport is given.
pub const SHEEP_A_SUPERCHUNK: &str = "sheep a superchunk";

/// Superchunks along the side of the viewport asked for.
pub const VIEWPORT_SIDE: &str = "viewport side";

/// Seconds each part is measured over.
pub const SECONDS: &str = "seconds";

/// Superchunks a frame carries at most.
pub const SUPERCHUNKS_A_FRAME: &str = "superchunks a frame";

/// Ticks a second the world is held to: 0, flat out.
pub const PACE: &str = "pace";

/// Seconds from one frame asked for to the next, at least, as a window
/// asks.
const BETWEEN_ASKS: Duration = Duration::from_micros(16_667);

/// How long the world is left alone once the viewport is hot, before anything is measured.
const SETTLING: Duration = Duration::from_secs(5);

/// One frame of `viewport`, `skip` of its hot superchunks passed over.
fn frame(host: &Host, frames: &Receiver<Frame>, viewport: Viewport, skip: u32, most: u32) -> Result<Frame, String> {
    if !host.sync(Ask { viewport: Some(viewport), detail: 8, skip, most, near: None }) {
        return Err("the host is gone".to_string());
    }
    whole_answer(frames)
}

/// Runs the probe: a line for the ticks with no frame asked, a line for
/// those with frames asked.
pub fn run(given: &Given) -> Result<(), String> {
    let (sheep, viewport_side, seconds, most, pace): (f32, u32, f64, u32, u32) = (given.number(SHEEP_A_SUPERCHUNK)?, given.number(VIEWPORT_SIDE)?, given.number(SECONDS)?, given.number(SUPERCHUNKS_A_FRAME)?, given.number(PACE)?);
    let mut tuning = defaults();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP]) = (0.0, 1.0, sheep);
    let (middle_x, middle_y) = WORLD_MIDDLE.cartesian();
    let first = (middle_x - viewport_side / 2, middle_y - viewport_side / 2);
    let viewport = Viewport { first, last: (first.0 + viewport_side.max(1) - 1, first.1 + viewport_side.max(1) - 1) };
    let (host, frames) = Host::start();
    host.pace(None);
    host.make_world(Start::from_tuning(seed(given)?, &tuning));
    let whole = frame_that(&host, &frames, Some(viewport), |frame| frame.hot.len() as u32 >= viewport_side * viewport_side)?;
    host.pace((pace != 0).then_some(pace));
    println!("# seed {}, {} superchunks hot in the viewport, {} sheep", utilities::seed::hex(whole.seed), whole.hot.len(), whole.sheep);
    // The copy of the whole viewport let go, and the world left to settle: what is measured is its ticks, not its making.
    drop(whole);
    std::thread::sleep(SETTLING);
    println!("frames asked,seconds,ticks a second,frames a second,seconds a frame took the host,share of the host's time");
    // No frame asked: one at each end, for the ticks between.
    let (from, began) = (frame(&host, &frames, viewport, 0, 0)?.tick, Instant::now());
    std::thread::sleep(Duration::from_secs_f64(seconds));
    let (to, took) = (frame(&host, &frames, viewport, 0, 0)?.tick, began.elapsed().as_secs_f64());
    println!("none,{took:.2},{:.1},0,0,0", (to - from) as f64 / took);
    // Frames asked as a window asks.
    let (began, mut skip, mut answered, mut host_seconds, mut last) = (Instant::now(), 0, 0u32, 0.0, to);
    while began.elapsed().as_secs_f64() < seconds {
        let asked = Instant::now();
        let frame = frame(&host, &frames, viewport, skip, most)?;
        skip = if skip + most < frame.hot.len() as u32 { skip + most } else { 0 };
        (answered, host_seconds, last) = (answered + 1, host_seconds + frame.sync_seconds, frame.tick);
        std::thread::sleep(BETWEEN_ASKS.saturating_sub(asked.elapsed()));
    }
    let took = began.elapsed().as_secs_f64();
    println!("as a window,{took:.2},{:.1},{:.1},{:.5},{:.3}", (last - to) as f64 / took, answered as f64 / took, host_seconds / answered.max(1) as f64, host_seconds / took);
    Ok(())
}
