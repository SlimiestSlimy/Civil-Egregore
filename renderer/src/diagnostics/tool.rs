//! The renderer's diagnostics tools, run by `Civil_Egregore renderer
//! <tool>`.

use super::stills::{self, Still};
use crate::frames::picture_of;
use crate::transient_data::renders;
use utilities::commands::{Command, Given, Parameter};

/// The seed the stills' world is made from, in hex: 0, the counted
/// one, moved on to one with land about the world's middle.
const SEED: &str = "seed";
/// Cells east of the world's middle the stills are centred on.
const CELLS_EAST: &str = "cells east of the middle";
/// Cells south of the world's middle the stills are centred on.
const CELLS_SOUTH: &str = "cells south of the middle";
/// The farthest still painted from cells: cells a pixel.
const FARTHEST: &str = "cells a pixel at the farthest";
/// What the stills' files are named after.
const NAMED: &str = "name";

/// Cells a screen pixel the window's still is taken at.
const CELLS_A_PIXEL: &str = "cells a screen pixel";

/// The renderer's tools.
pub const COMMANDS: [Command; 2] = [Command {
    name: "stills",
    does: "paints one place of a world at every zoom -- the map, the cells from far, a cell a pixel, the cells from near -- with no window, and keeps each as a PNG under the renderer's transient_data/renders/",
    parameters: &[Parameter::new(SEED, "0"), Parameter::new(CELLS_EAST, "0"), Parameter::new(CELLS_SOUTH, "0"), Parameter::new(FARTHEST, "8"), Parameter::new(NAMED, "still")],
    run: stills_tool,
}, Command {
    name: "window_still",
    does: "opens the window on a world, the view so many cells a screen pixel over its middle, keeps what the graphics card drew -- the pictures' mipmaps among it -- as a PNG under the renderer's transient_data/renders/, and closes it",
    parameters: &[Parameter::new(SEED, "0"), Parameter::new(CELLS_A_PIXEL, "1.5"), Parameter::new(NAMED, "window")],
    run: window_still_tool,
}];

/// Opens the window, keeps its still and closes it, a line printed for
/// the file.
fn window_still_tool(given: &Given) -> Result<(), String> {
    let text = given.text(SEED)?;
    let seed = utilities::seed::of_hex(text).ok_or_else(|| format!("`{text}` is no seed in hex"))?;
    let seed = if seed == 0 { utilities::seed::counted() } else { seed };
    let cells_a_pixel: f32 = given.number(CELLS_A_PIXEL)?;
    let (folder, named) = (renders(), given.text(NAMED)?);
    std::fs::create_dir_all(&folder).map_err(|why| format!("{}: {why}", folder.display()))?;
    let path = folder.join(format!("{named}_{cells_a_pixel}.png"));
    super::window_still::keep(seed, cells_a_pixel, path.clone());
    println!("still,file\nwindow,{}\n# seed {}", path.display(), utilities::seed::hex(seed));
    Ok(())
}

/// Paints the stills and keeps them, a line printed for each.
fn stills_tool(given: &Given) -> Result<(), String> {
    let text = given.text(SEED)?;
    let seed = utilities::seed::of_hex(text).ok_or_else(|| format!("`{text}` is no seed in hex"))?;
    let (east, south, farthest): (i64, i64, u32) = (given.number(CELLS_EAST)?, given.number(CELLS_SOUTH)?, given.number(FARTHEST)?);
    let (folder, named) = (renders(), given.text(NAMED)?);
    std::fs::create_dir_all(&folder).map_err(|why| format!("{}: {why}", folder.display()))?;
    println!("still,file");
    let seed = stills::gather((seed != 0).then_some(seed), (east, south), farthest, |Still { name, size, pixels }| {
        let path = folder.join(format!("{named}_{name}.png"));
        let image = picture_of(size, pixels).try_into_dynamic().map_err(|why| format!("{name}: {why}"))?;
        image.save(&path).map_err(|why| format!("{}: {why}", path.display()))?;
        println!("{name},{}", path.display());
        Ok(())
    })?;
    println!("# seed {}", utilities::seed::hex(seed));
    Ok(())
}
