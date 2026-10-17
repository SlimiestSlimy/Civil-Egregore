//! What answering a client costs the simulation: a world whose camera
//! loads, a wide viewport kept hot, and its ticks a second while no
//! frame is asked for and while frames are asked for as a window asks
//! -- one at a time, sixty a second at most, going round the viewport.

use crate::asking_the_host::{frame_that, seed, whole_answer};
use coordinates::WORLD_MIDDLE;
use server::host::frame::{FrameAsk, ViewportFrame, Viewport};
use server::host::Host;
use server::WorldStart;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use crate::ProbeParameters;
use utilities::tuning::{default_tuning, CAMERA_LOADS, SHEEP_PER_SUPERCHUNK, WORLD_SIDE};

/// Sheep each superchunk generated in the viewport is given.
pub const SHEEP_A_SUPERCHUNK: &str = "sheep";

/// Superchunks along the side of the viewport asked for.
pub const VIEWPORT_SIDE: &str = "viewport_side";

/// Seconds each part is measured over.
pub const SECONDS_MEASURED: &str = "seconds";

/// Superchunks a frame carries at most.
pub const SUPERCHUNKS_A_FRAME: &str = "superchunks_a_frame";

/// Ticks a second the world is held to: 0, flat out.
pub const PACE_TICKS_A_SECOND: &str = "pace";

/// Seconds from one frame asked for to the next, at least, as a window
/// asks.
const BETWEEN_ASKS: Duration = Duration::from_micros(16_667);

/// How long the world is left alone once the viewport is hot, before anything is measured.
const SETTLING_TIME: Duration = Duration::from_secs(5);

/// One frame of `viewport`, `skipped_superchunks` of its hot superchunks passed over.
fn ask_one_frame(host: &Host, frame_receiver: &Receiver<ViewportFrame>, viewport: Viewport, skipped_superchunks: u32, most_superchunks: u32) -> Result<ViewportFrame, String> {
    if !host.ask_frame(FrameAsk { viewport: Some(viewport), pixel_side_log2: 8, superchunks_skipped: skipped_superchunks, most_superchunks, seen_near: None }) {
        return Err("the host is gone".to_string());
    }
    whole_answer(frame_receiver)
}

/// Runs the probe: a line for the ticks with no frame asked, a line for
/// those with frames asked.
pub fn run_probe(given_parameters: &ProbeParameters) -> Result<(), String> {
    let (sheep_a_superchunk, viewport_side, seconds_measured, most_superchunks, pace_ticks_a_second): (f32, u32, f64, u32, u32) = (given_parameters.argument_number(SHEEP_A_SUPERCHUNK)?, given_parameters.argument_number(VIEWPORT_SIDE)?, given_parameters.argument_number(SECONDS_MEASURED)?, given_parameters.argument_number(SUPERCHUNKS_A_FRAME)?, given_parameters.argument_number(PACE_TICKS_A_SECOND)?);
    let mut tuning = default_tuning();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP_PER_SUPERCHUNK]) = (0.0, 1.0, sheep_a_superchunk);
    let (middle_x, middle_y) = WORLD_MIDDLE.cartesian();
    let viewport_first_cartesian = (middle_x - viewport_side / 2, middle_y - viewport_side / 2);
    let viewport = Viewport { top_left_superchunk_cartesian: viewport_first_cartesian, bottom_right_superchunk_cartesian: (viewport_first_cartesian.0 + viewport_side.max(1) - 1, viewport_first_cartesian.1 + viewport_side.max(1) - 1) };
    let (host, frame_receiver) = Host::start_on_thread();
    host.set_pace(None);
    host.make_world(WorldStart::from_tuning(seed(given_parameters)?, &tuning));
    let whole_frame = frame_that(&host, &frame_receiver, Some(viewport), |answered_frame| answered_frame.hot.len() as u32 >= viewport_side * viewport_side)?;
    host.set_pace((pace_ticks_a_second != 0).then_some(pace_ticks_a_second));
    println!("# seed {}, {} superchunks hot in the viewport, {} sheep", utilities::seed::seed_as_hex(whole_frame.seed), whole_frame.hot.len(), whole_frame.sheep_count);
    // The copy of the whole viewport let go, and the world left to settle: what is measured is its ticks, not its making.
    drop(whole_frame);
    std::thread::sleep(SETTLING_TIME);
    println!("frames asked,seconds,ticks a second,frames a second,seconds a frame took the host,share of the host's time");
    // No frame asked: one at each end, for the ticks between.
    let (tick_from, began_quiet) = (ask_one_frame(&host, &frame_receiver, viewport, 0, 0)?.tick, Instant::now());
    std::thread::sleep(Duration::from_secs_f64(seconds_measured));
    let (tick_to, quiet_seconds) = (ask_one_frame(&host, &frame_receiver, viewport, 0, 0)?.tick, began_quiet.elapsed().as_secs_f64());
    println!("none,{quiet_seconds:.2},{:.1},0,0,0", (tick_to - tick_from) as f64 / quiet_seconds);
    // Frames asked as a window asks.
    let (began_asking, mut skipped_superchunks, mut frames_answered, mut host_seconds, mut last_tick) = (Instant::now(), 0, 0u32, 0.0, tick_to);
    while began_asking.elapsed().as_secs_f64() < seconds_measured {
        let asked_at = Instant::now();
        let asked_frame = ask_one_frame(&host, &frame_receiver, viewport, skipped_superchunks, most_superchunks)?;
        skipped_superchunks = if skipped_superchunks + most_superchunks < asked_frame.hot.len() as u32 { skipped_superchunks + most_superchunks } else { 0 };
        (frames_answered, host_seconds, last_tick) = (frames_answered + 1, host_seconds + asked_frame.sync_seconds, asked_frame.tick);
        std::thread::sleep(BETWEEN_ASKS.saturating_sub(asked_at.elapsed()));
    }
    let asking_seconds = began_asking.elapsed().as_secs_f64();
    println!("as a window,{asking_seconds:.2},{:.1},{:.1},{:.5},{:.3}", (last_tick - tick_to) as f64 / asking_seconds, frames_answered as f64 / asking_seconds, host_seconds / frames_answered.max(1) as f64, host_seconds / asking_seconds);
    Ok(())
}
