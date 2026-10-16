//! The renderer's diagnostics tools, run by `Civil_Egregore renderer
//! <tool>`.

use super::stills::{self, StillPicture};
use crate::frames::picture_of;
use crate::transient_data::renders_folder;
use utilities::commands::{NamedCommand, GivenCommand, CommandParameter};

/// The seed the stills' world is made from, in hex: 0, the counted
/// one, moved on to one with land about the world's middle.
const SEED: &str = "seed";
/// Cells east of the world's middle the stills are centred on.
const CELLS_EAST: &str = "cells east of the middle";
/// Cells south of the world's middle the stills are centred on.
const CELLS_SOUTH: &str = "cells south of the middle";
/// The farthest still painted from cells: cells a pixel.
const CELLS_A_PIXEL_FARTHEST: &str = "cells a pixel at the farthest";
/// What the stills' files are named after.
const NAMED_AFTER: &str = "name";

/// Cells a screen pixel the window's still is taken at.
const CELLS_A_PIXEL: &str = "cells a screen pixel";

/// The renderer's tools.
pub const COMMANDS: [NamedCommand; 4] = [NamedCommand {
    command_name: "stills",
    what_it_does: "paints one place of a world at every zoom -- the map, the cells from far, a cell a pixel, the cells from near -- with no window, and keeps each as a PNG under the renderer's transient_data/renders/",
    command_parameters: &[CommandParameter::new(SEED, "0"), CommandParameter::new(CELLS_EAST, "0"), CommandParameter::new(CELLS_SOUTH, "0"), CommandParameter::new(CELLS_A_PIXEL_FARTHEST, "8"), CommandParameter::new(NAMED_AFTER, "still")],
    run_function: stills_tool,
}, NamedCommand {
    command_name: "window_still",
    what_it_does: "opens the window on a world, the view so many cells a screen pixel over its middle, keeps what the graphics card drew -- the pictures' mipmaps among it -- as a PNG under the renderer's transient_data/renders/, and closes it",
    command_parameters: &[CommandParameter::new(SEED, "0"), CommandParameter::new(CELLS_A_PIXEL, "1.5"), CommandParameter::new(NAMED_AFTER, "window")],
    run_function: window_still_tool,
}, NamedCommand {
    command_name: "watch",
    what_it_does: "opens the window on a world an in-bin test ticks, running, to be watched: the world `server check` ticks from a seed -- 0, the run's own, the seed the tests are on -- or, given superchunks, the plain `server pasture` ticks, with its grass",
    command_parameters: &[CommandParameter::new(SEED, "0"), CommandParameter::new(SHEEP_WATCHED, "4000"), CommandParameter::new(PASTURE_SUPERCHUNKS, "0"), CommandParameter::new(GRASS_THOUSANDTHS, "333")],
    run_function: watch_tool,
}, NamedCommand {
    command_name: "show",
    what_it_does: "opens the window that shows worlds as they are ticked: the world of every test and every in-bin test that ticks one while CIVIL_EGREGORE_RENDER is set is drawn in it, one after another, flat out unless the keys pause or pace it; a test or a diagnostic that finds none open starts it",
    command_parameters: &[],
    run_function: |_| super::window_on::show_worlds(),
}];

/// Sheep the world watched starts with: on its origin, or on each
/// superchunk of a pasture.
const SHEEP_WATCHED: &str = "sheep";
/// Superchunks of the pasture watched: 0, no pasture but the world of
/// the seed.
const PASTURE_SUPERCHUNKS: &str = "pasture superchunks";
/// The share of a pasture's cells that are grass, in thousandths.
const GRASS_THOUSANDTHS: &str = "grass, thousandths";

/// Opens the window on the world `server check` ticks from the seed
/// given, or on the plain `server pasture` ticks, and runs until it is
/// closed.
fn watch_tool(given_parameters: &GivenCommand) -> Result<(), String> {
    let seed_text = given_parameters.argument_text(SEED)?;
    let seed = utilities::seed::of_hex(seed_text).ok_or_else(|| format!("`{seed_text}` is no seed in hex"))?;
    let (sheep_count, superchunks, grass_thousandths): (usize, u32, u64) = (given_parameters.argument_number(SHEEP_WATCHED)?, given_parameters.argument_number(PASTURE_SUPERCHUNKS)?, given_parameters.argument_number(GRASS_THOUSANDTHS)?);
    let world_start = if superchunks > 0 {
        println!("# the pasture: {superchunks} superchunks, {sheep_count} sheep on each, grass on {grass_thousandths} thousandths");
        server::diagnostics::plain_world::pasture_start(superchunks, grass_thousandths, sheep_count)
    } else {
        // As the tests have it: the run's seed moved on to one with land about the origin, where the sheep start.
        let seed = if seed == 0 { server::seed_with_land(utilities::seed::counted_seed(), &server::WorldGeneration::DEFAULT) } else { seed };
        println!("# seed {}, {sheep_count} sheep", utilities::seed::seed_as_hex(seed));
        server::WorldStart { seed, sheep_per_flock: sheep_count, ..server::WorldStart::default() }
    };
    super::window_on::open_window(world_start);
    Ok(())
}

/// Opens the window, keeps its still and closes it, a line printed for
/// the file.
fn window_still_tool(given_parameters: &GivenCommand) -> Result<(), String> {
    let seed_text = given_parameters.argument_text(SEED)?;
    let seed = utilities::seed::of_hex(seed_text).ok_or_else(|| format!("`{seed_text}` is no seed in hex"))?;
    let seed = if seed == 0 { utilities::seed::counted_seed() } else { seed };
    let cells_a_pixel: f32 = given_parameters.argument_number(CELLS_A_PIXEL)?;
    let (folder, named_after) = (renders_folder(), given_parameters.argument_text(NAMED_AFTER)?);
    std::fs::create_dir_all(&folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))?;
    let path = folder.join(format!("{named_after}_{cells_a_pixel}.png"));
    super::window_still::save_still(seed, cells_a_pixel, path.clone());
    println!("still,file\nwindow,{}\n# seed {}", path.display(), utilities::seed::seed_as_hex(seed));
    Ok(())
}

/// Paints the stills and keeps them, a line printed for each.
fn stills_tool(given_parameters: &GivenCommand) -> Result<(), String> {
    let seed_text = given_parameters.argument_text(SEED)?;
    let seed = utilities::seed::of_hex(seed_text).ok_or_else(|| format!("`{seed_text}` is no seed in hex"))?;
    let (cells_east, cells_south, farthest_cells_a_pixel): (i64, i64, u32) = (given_parameters.argument_number(CELLS_EAST)?, given_parameters.argument_number(CELLS_SOUTH)?, given_parameters.argument_number(CELLS_A_PIXEL_FARTHEST)?);
    let (folder, named_after) = (renders_folder(), given_parameters.argument_text(NAMED_AFTER)?);
    std::fs::create_dir_all(&folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))?;
    println!("still,file");
    let seed = stills::gather_stills((seed != 0).then_some(seed), (cells_east, cells_south), farthest_cells_a_pixel, |StillPicture { zoom_name: name, size_pixels: size, rgba_bytes: pixels }| {
        let path = folder.join(format!("{named_after}_{name}.png"));
        let image = picture_of(size, pixels).try_into_dynamic().map_err(|image_error| format!("{name}: {image_error}"))?;
        image.save(&path).map_err(|save_error| format!("{}: {save_error}", path.display()))?;
        println!("{name},{}", path.display());
        Ok(())
    })?;
    println!("# seed {}", utilities::seed::seed_as_hex(seed));
    Ok(())
}
