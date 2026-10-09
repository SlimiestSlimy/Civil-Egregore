//! Whether a world reset is whole at once, and where the tree rule
//! takes a cover generated: the trees of a world forced hot, as it
//! ticks.

use crate::asking_the_host::{frame_that, seed};
use server::host::Host;
use server::Start;
use utilities::commands::Given;
use utilities::tuning::{defaults, FORCED_HOT, SHEEP, TREE_COVER, WORLD_SIDE};

/// The tree cover the world is made with.
pub const COVER: &str = "tree cover";

/// The tree cover the world is reset to before it ticks, if any.
pub const COVER_RESET_TO: &str = "tree cover reset to";

/// Ticks the world is run for.
pub const TICKS: &str = "ticks";

/// Superchunks along the world's side.
pub const SIDE: &str = "side";

/// Lines printed over the run, after the first.
const LINES: u64 = 10;

/// Runs the probe: a line as the world is made, one as it is reset, and
/// [`LINES`] as it ticks.
pub fn run(given: &Given) -> Result<(), String> {
    let (ticks, side): (u64, f32) = (given.number(TICKS)?, given.number(SIDE)?);
    let mut tuning = defaults();
    (tuning[TREE_COVER], tuning[WORLD_SIDE], tuning[FORCED_HOT], tuning[SHEEP]) = (given.number(COVER)?, side, 1.0, 0.0);
    let (host, frames) = Host::start();
    host.pause(true);
    host.pace(None);
    let start = Start::from_tuning(seed(given)?, &tuning);
    host.make_world(start);
    let made = frame_that(&host, &frames, None, |_| true)?;
    println!("# seed {}", utilities::seed::hex(made.seed));
    println!("what,tick,trees,grass");
    println!("made,{},{},{}", made.tick, made.trees, made.grass);
    let reset_to = given.text(COVER_RESET_TO)?;
    if !reset_to.is_empty() {
        tuning[TREE_COVER] = reset_to.parse().map_err(|_| format!("`{reset_to}` is no tree cover"))?;
        host.reset(&tuning);
        let reset = frame_that(&host, &frames, None, |frame| frame.world > made.world)?;
        println!("reset,{},{},{}", reset.tick, reset.trees, reset.grass);
    }
    host.pause(false);
    for line in 1..=LINES {
        let ticked = frame_that(&host, &frames, None, |frame| frame.tick >= ticks * line / LINES)?;
        println!("ticked,{},{},{}", ticked.tick, ticked.trees, ticked.grass);
    }
    Ok(())
}
