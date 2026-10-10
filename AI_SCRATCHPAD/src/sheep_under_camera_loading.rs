//! Whether every superchunk a camera generates is given its flock: the
//! sheep on each hot superchunk of a viewport, in a world with no side.

use crate::asking_the_host::{frame_that, seed};
use coordinates::WORLD_MIDDLE;
use server::host::frame::Viewport;
use server::host::Host;
use server::Start;
use utilities::commands::Given;
use utilities::tuning::{defaults, CAMERA_LOADS, SHEEP, WORLD_SIDE};

/// Sheep each superchunk generated in the viewport is given.
pub const SHEEP_A_SUPERCHUNK: &str = "sheep a superchunk";

/// Superchunks along the side of the viewport asked for.
pub const VIEWPORT_SIDE: &str = "viewport side";

/// Ticks waited before the sheep are counted: the camera's superchunks
/// are given their flocks as the halos move, a tick after they are asked.
pub const TICKS_WAITED: &str = "ticks waited";

/// Runs the probe: a line a hot superchunk of the viewport, once every
/// one of it is hot.
pub fn run(given: &Given) -> Result<(), String> {
    let (sheep, viewport_side, ticks_waited): (f32, u32, u64) = (given.number(SHEEP_A_SUPERCHUNK)?, given.number(VIEWPORT_SIDE)?, given.number(TICKS_WAITED)?);
    let mut tuning = defaults();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP]) = (0.0, 1.0, sheep);
    let (middle_x, middle_y) = WORLD_MIDDLE.cartesian();
    let first = (middle_x - viewport_side / 2, middle_y - viewport_side / 2);
    let viewport = Viewport { first, last: (first.0 + viewport_side.max(1) - 1, first.1 + viewport_side.max(1) - 1) };
    let (host, frames) = Host::start();
    host.pace(None);
    host.make_world(Start::from_tuning(seed(given)?, &tuning));
    let frame = frame_that(&host, &frames, Some(viewport), |frame| frame.tick >= ticks_waited && frame.hot.len() as u32 >= viewport_side * viewport_side && frame.cells.len() == frame.hot.len())?;
    println!("# seed {}, tick {}, {} sheep in the world", utilities::seed::hex(frame.seed), frame.tick, frame.sheep);
    println!("superchunk x,superchunk y,sheep");
    for cells in &frame.cells {
        println!("{},{},{}", cells.at.0 - first.0, cells.at.1 - first.1, cells.sheep.len());
    }
    Ok(())
}
