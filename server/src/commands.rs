//! The server's commands, as [`crate::start`] runs them from the clean
//! parameters ([`crate::params`]): a world made in a folder, run and
//! looked at (`docs/reference.md`, "commands.rs").

use crate::params::{FOLDER, FORCED_HOT, SIDE, THREADS, TICKS};
use crate::HaloChange;
use crate::chunk_storage::disk;
use protocol::server_parameters::CleanServerParameters;
use protocol::type_registry::FrozenTypes;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// The world's folder the command was given: a name in the worlds'
/// folder, or a path.
fn folder_of(command_parameters: &CleanServerParameters) -> PathBuf {
    utilities::settings::world(command_parameters.text(FOLDER).expect("every command on a folder is given one"))
}

/// The name of the world in `folder`: the folder's.
fn world_name_of(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |folder_file_name| folder_file_name.to_string_lossy().into_owned())
}

/// The count `parameter_name`, or `default_count` if not given.
fn count_of(command_parameters: &CleanServerParameters, parameter_name: &str, default_count: u64) -> Result<u64, String> {
    command_parameters.integer(parameter_name).map_or(Ok(default_count), |given_count| u64::try_from(given_count).map_err(|_| format!("--{parameter_name} {given_count}: a count is never below 0")))
}

/// Makes a world of `frozen_types` from `seed` and saves it in the
/// folder, as the command's parameters say: its side, whether forced
/// hot, and threads; generated as its generation has it unless told
/// otherwise (`docs/reference.md`, "commands.rs").
pub fn new(seed: u64, command_parameters: &CleanServerParameters, frozen_types: &Arc<FrozenTypes>) -> Result<String, String> {
    let folder = folder_of(command_parameters);
    let folder = folder.as_path();
    if folder.join(disk::WORLD_FILE).exists() {
        return Err(format!("{} is a world already", folder.display()));
    }
    let forced_hot = command_parameters.nested(FORCED_HOT).is_some();
    let world_size = crate::WorldSize::of_side(count_of(command_parameters, SIDE, 0)? as u32, forced_hot)?;
    let thread_count = Some(count_of(command_parameters, THREADS, 0)? as usize).filter(|&asked_threads| asked_threads > 0);
    let mut made_world = crate::start_world(crate::WorldStart { seed, world_size, thread_count, ..crate::WorldStart::default() }, frozen_types);
    let save_written = crate::save(folder, &mut made_world).map_err(|save_error| save_error.to_string())?;
    Ok(format!("{}, seed {}: {} superchunks, {} entities, {} bytes in {}", world_name_of(folder), utilities::seed::seed_as_hex(seed), save_written.superchunks, save_written.entities_written, save_written.bytes_written, folder.display()))
}

/// Loads the world in the folder, its types among `available`, ticks
/// it, and saves it.
pub fn run_saved_world(command_parameters: &CleanServerParameters, available: &FrozenTypes) -> Result<String, String> {
    let folder = folder_of(command_parameters);
    let folder = folder.as_path();
    let ticks = count_of(command_parameters, TICKS, 10_000)?;
    let mut loaded_world = crate::load_world(folder, available).map_err(|load_error| load_error.to_string())?;
    let ticking_began = Instant::now();
    let mut halos = HaloChange::default();
    for _ in 0..ticks {
        halos += loaded_world.tick().halos;
    }
    let ticking_seconds = ticking_began.elapsed().as_secs_f64();
    let hot = loaded_world.bitmap_arena().superchunks().len();
    let save_written = crate::save(folder, &mut loaded_world).map_err(|save_error| save_error.to_string())?;
    let (warming, cooling) = (loaded_world.warming().count(), loaded_world.cooling().count());
    let HaloChange { superchunks_reached: reached, superchunks_generated: generated, superchunks_restored: restored, superchunks_cooled: cooled } = halos;
    Ok(format!(
        "{}: tick {} -> {}, {:.0} ticks a second; {} entities; {hot} of {} superchunks hot, {cooling} of them cooling, {warming} warming; superchunks {reached} reached, {generated} generated, {restored} restored, {cooled} cooled; {} bytes saved",
        world_name_of(folder),
        loaded_world.world_info.tick,
        loaded_world.entity_store().current_tick(),
        ticks as f64 / ticking_seconds,
        save_written.entities_written,
        save_written.superchunks,
        save_written.bytes_written
    ))
}

/// Says what the world in the folder is.
pub fn describe_saved_world(command_parameters: &CleanServerParameters) -> Result<String, String> {
    let folder = folder_of(command_parameters);
    let folder = folder.as_path();
    let world_info = disk::read_world(folder).map_err(|read_error| read_error.to_string())?;
    let superchunks = disk::saved_superchunks(folder).map_err(|list_error| list_error.to_string())?;
    let size_text = world_info.side_in_superchunks.map_or("of no size".to_string(), |side_superchunks| format!("{side_superchunks} superchunks a side"));
    Ok(format!("{}: seed {}, at tick {}, {size_text}, {} superchunks, layer types {:?}", world_name_of(folder), utilities::seed::seed_as_hex(world_info.seed), world_info.tick, superchunks.len(), world_info.layers.iter().map(|layer| layer.0).collect::<Vec<_>>()))
}
