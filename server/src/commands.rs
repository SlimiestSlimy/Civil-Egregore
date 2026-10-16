//! The server's commands ([`COMMANDS`]), run by `Civil_Egregore server
//! <command>`: a world made in a folder, run and looked at, and the
//! diagnostics tools (`docs/reference.md`, "commands.rs").

use crate::diagnostics::tool::{print_world_hashes, print_pasture, print_throughput, TICKS_BETWEEN_HASHES, FLOCK, GRASS_THOUSANDTHS as GRASS_SHARE, SEED, ORIGIN_SHEEP, SUPERCHUNKS, THREAD_COUNT, TICKS};
use crate::HaloChange;
use chunk_storage::disk;
use type_registry::GRASS;
use std::path::Path;
use std::time::Instant;
use utilities::commands::{NamedCommand, GivenCommand, CommandParameter};

/// A world's folder: a name in the worlds' folder, or a path.
const FOLDER: &str = "folder";

/// Whether a world is made forced hot throughout, not about its
/// flock's halo: 0, halo; anything else, forced.
const FORCED_HOT_PARAMETER: &str = "forced";

/// The server's commands: a world made, run and looked at, and its
/// diagnostics tools.
pub const COMMANDS: [NamedCommand; 6] = [
    NamedCommand {
        command_name: "new",
        what_it_does: "makes a world from a seed and saves it in the folder: so many superchunks along a side, or 0 for as far as it goes; so many sheep on every superchunk of a world with a side, on its origin of one without; forced hot throughout if told -- only with a side -- else about the sheep's halos; on so many threads, or every one the machine has if 0",
        command_parameters: &[CommandParameter::new(FOLDER, ""), CommandParameter::new("seed", "1"), CommandParameter::new("sheep", ""), CommandParameter::new("side", "0"), CommandParameter::new(FORCED_HOT_PARAMETER, "0"), CommandParameter::new(THREAD_COUNT, "0")],
        run_function: |given_arguments| print_command_on_folder(given_arguments, new),
    },
    NamedCommand { command_name: "run", what_it_does: "loads the world in the folder, ticks it, and saves it", command_parameters: &[CommandParameter::new(FOLDER, ""), CommandParameter::new("ticks", "10000")], run_function: |given_arguments| print_command_on_folder(given_arguments, run_saved_world) },
    NamedCommand { command_name: "info", what_it_does: "says what the world in the folder is", command_parameters: &[CommandParameter::new(FOLDER, "")], run_function: |given_arguments| print_command_on_folder(given_arguments, |folder, _| describe_saved_world(folder)) },
    NamedCommand {
        command_name: "throughput",
        what_it_does: "ticks grass flat out: each phase's time, the writes a second, the memory held",
        command_parameters: &[CommandParameter::new(TICKS, "500"), CommandParameter::new(GRASS_SHARE, "333"), CommandParameter::new(SUPERCHUNKS, "16"), CommandParameter::new(THREAD_COUNT, "0")],
        run_function: print_throughput,
    },
    NamedCommand {
        command_name: "pasture",
        what_it_does: "ticks grass and sheep flat out: the flock, what the sheep did, each rule's time, the memory held",
        command_parameters: &[CommandParameter::new(TICKS, "2000"), CommandParameter::new(GRASS_SHARE, "333"), CommandParameter::new(FLOCK, "4000"), CommandParameter::new(SUPERCHUNKS, "16"), CommandParameter::new(THREAD_COUNT, "0")],
        run_function: print_pasture,
    },
    NamedCommand {
        command_name: "check",
        what_it_does: "ticks a world from a seed and prints its hash every so many ticks, part by part: the same rows on every machine and any number of threads, if the simulation is deterministic",
        command_parameters: &[CommandParameter::new(SEED, "0"), CommandParameter::new(TICKS, "2000"), CommandParameter::new(TICKS_BETWEEN_HASHES, "100"), CommandParameter::new(ORIGIN_SHEEP, "4000"), CommandParameter::new(THREAD_COUNT, "0")],
        run_function: print_world_hashes,
    },
];

/// Runs `folder_command` on the folder given and what follows it, and prints
/// the line it gives.
fn print_command_on_folder(given_arguments: &GivenCommand, folder_command: impl FnOnce(&Path, &[&str]) -> Result<String, String>) -> Result<(), String> {
    let folder = given_arguments.argument_text(FOLDER)?;
    println!("{}", folder_command(&utilities::settings::world(folder), &given_arguments.argument_words()[1..])?);
    Ok(())
}

/// The name of the world in `folder`: the folder's.
fn world_name_of(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |folder_file_name| folder_file_name.to_string_lossy().into_owned())
}

/// `given_argument` as a number, or `default_number` if not given.
fn number_argument(given_argument: Option<&&str>, default_number: u64) -> Result<u64, String> {
    given_argument.map_or(Ok(default_number), |argument_text| argument_text.parse().map_err(|_| format!("`{argument_text}` is not a number")))
}

/// Makes a world from a seed and saves it in `folder`, as the rest of
/// the line says: its sheep, side, whether forced hot, and threads
/// (`docs/reference.md`, "commands.rs").
pub fn new(folder: &Path, rest_of_line: &[&str]) -> Result<String, String> {
    if folder.join(disk::WORLD_FILE).exists() {
        return Err(format!("{} is a world already", folder.display()));
    }
    let seed = rest_of_line.first().map_or(Ok(1), |seed| utilities::seed::of_hex(seed).ok_or_else(|| format!("`{seed}` is not a seed: 64 bits, in hexadecimal")))?;
    let sheep_per_flock = number_argument(rest_of_line.get(1), crate::FLOCK as u64)? as usize;
    let forced_hot = number_argument(rest_of_line.get(3), 0)? != 0;
    let world_size = crate::WorldSize::of_side(number_argument(rest_of_line.get(2), 0)? as u32, forced_hot)?;
    let thread_count = Some(number_argument(rest_of_line.get(4), 0)? as usize).filter(|&asked_threads| asked_threads > 0);
    let mut made_world = crate::start_world(crate::WorldStart { seed, world_size, thread_count, sheep_per_flock, ..crate::WorldStart::default() });
    let save_written = crate::save(folder, &mut made_world).map_err(|save_error| save_error.to_string())?;
    Ok(format!("{}, seed {}: {} superchunks, {} entities, {} bytes in {}", world_name_of(folder), utilities::seed::seed_as_hex(seed), save_written.superchunks, save_written.entities_written, save_written.bytes_written, folder.display()))
}

/// Loads the world in `folder`, ticks it, and saves it.
pub fn run_saved_world(folder: &Path, rest_of_line: &[&str]) -> Result<String, String> {
    let ticks = number_argument(rest_of_line.first(), 10_000)?;
    let mut loaded_world = crate::load_world(folder).map_err(|load_error| load_error.to_string())?;
    let ticking_began = Instant::now();
    let mut halos = HaloChange::default();
    for _ in 0..ticks {
        halos += loaded_world.tick().halos;
    }
    let ticking_seconds = ticking_began.elapsed().as_secs_f64();
    let hot = loaded_world.bitmap_arena.superchunks().len();
    let grass_cells: u64 = loaded_world.bitmap_arena.superchunks().iter().map(|superchunk| loaded_world.bitmap_arena.superchunk_count(GRASS, superchunk.superchunk_index()) as u64).sum();
    let save_written = crate::save(folder, &mut loaded_world).map_err(|save_error| save_error.to_string())?;
    let (warming, cooling) = (loaded_world.warming().count(), loaded_world.cooling().count());
    let HaloChange { superchunks_reached: reached, superchunks_generated: generated, superchunks_restored: restored, superchunks_cooled: cooled } = halos;
    Ok(format!(
        "{}: tick {} -> {}, {:.0} ticks a second; {} entities; {hot} of {} superchunks hot, {cooling} of them cooling, {warming} warming, {grass_cells} cells of grass on them; superchunks {reached} reached, {generated} generated, {restored} restored, {cooled} cooled; {} bytes saved",
        world_name_of(folder),
        loaded_world.world_info.tick,
        loaded_world.entity_store.current_tick(),
        ticks as f64 / ticking_seconds,
        save_written.entities_written,
        save_written.superchunks,
        save_written.bytes_written
    ))
}

/// Says what the world in `folder` is.
pub fn describe_saved_world(folder: &Path) -> Result<String, String> {
    let world_info = disk::read_world(folder).map_err(|read_error| read_error.to_string())?;
    let superchunks = disk::saved_superchunks(folder).map_err(|list_error| list_error.to_string())?;
    let size_text = world_info.side_in_superchunks.map_or("of no size".to_string(), |side_superchunks| format!("{side_superchunks} superchunks a side"));
    Ok(format!("{}: seed {}, at tick {}, {size_text}, {} superchunks, layer types {:?}", world_name_of(folder), utilities::seed::seed_as_hex(world_info.seed), world_info.tick, superchunks.len(), world_info.layers.iter().map(|layer| layer.0).collect::<Vec<_>>()))
}
