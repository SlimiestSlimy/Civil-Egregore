//! Where the sheep of a camera's flocks go: a world with no side whose
//! camera loads, ticked with no host between, and what its ticks say
//! became of its entities.

use coordinates::WORLD_MIDDLE;
use server::host::frame::Viewport;
use server::Start;
use utilities::commands::Given;
use utilities::tuning::{defaults, CAMERA_LOADS, SHEEP, WORLD_SIDE};

/// Sheep each superchunk generated in the viewport is given.
pub const SHEEP_A_SUPERCHUNK: &str = "sheep a superchunk";

/// Superchunks along the side of the viewport kept.
pub const VIEWPORT_SIDE: &str = "viewport side";

/// Ticks run.
pub const TICKS: &str = "ticks";

/// Lines printed over the run.
const LINES: u64 = 10;

/// Runs the probe: a line every tenth of the run, each what the ticks
/// since the last said.
pub fn run(given: &Given) -> Result<(), String> {
    let (sheep, viewport_side, ticks): (f32, u32, u64) = (given.number(SHEEP_A_SUPERCHUNK)?, given.number(VIEWPORT_SIDE)?, given.number(TICKS)?);
    let mut tuning = defaults();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP]) = (0.0, 1.0, sheep);
    let (middle_x, middle_y) = WORLD_MIDDLE.cartesian();
    let first = (middle_x - viewport_side / 2, middle_y - viewport_side / 2);
    let mut world = server::start(Start::from_tuning(crate::asking_the_host::seed(given)?, &tuning));
    world.halos.keep_viewport(Some(Viewport { first, last: (first.0 + viewport_side.max(1) - 1, first.1 + viewport_side.max(1) - 1) }));
    println!("# seed {}", utilities::seed::hex(world.info.seed));
    println!("tick,sheep,hot superchunks,puts,removes,lost,stayed,refused,crossed,births,deaths");
    let (mut puts, mut removes, mut lost, mut stayed, mut refused, mut crossed, mut births, mut deaths) = (0, 0, 0, 0, 0, 0, 0, 0);
    for tick in 1..=ticks {
        let report = world.tick().rules;
        let (applied, done) = (report.instructions_applied, report.rules.sheep);
        (puts, removes, lost, stayed, refused, crossed, births, deaths) = (puts + applied.puts, removes + applied.removes, lost + applied.lost, stayed + applied.stayed, refused + applied.refused, crossed + applied.crossed, births + done.births, deaths + done.deaths);
        if tick % (ticks / LINES).max(1) == 0 {
            println!("{tick},{},{},{puts},{removes},{lost},{stayed},{refused},{crossed},{births},{deaths}", world.entities.len(), world.entities.superchunks().len());
            (puts, removes, lost, stayed, refused, crossed, births, deaths) = (0, 0, 0, 0, 0, 0, 0, 0);
        }
    }
    Ok(())
}
