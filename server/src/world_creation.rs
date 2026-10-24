//! A world made in three steps, each on its folder: created -- its
//! folders and its world file, nothing generated ([`create_world`]) --
//! set up -- its type registry made for the first time and kept in its
//! types file, no library loaded nor any code of a type run
//! ([`set_up_world`]) -- and run, its libraries' code at work from then:
//! generated from its middle the first time, as it was saved every other
//! ([`run_world`]). A world is its folder from the first step: none is
//! held in memory alone.

use crate::chunk_storage::disk::{self, DiskError, WorldInfo, HOT_FILE, WORLD_FILE};
use crate::simulation::simulation_api::SetUpWorldgen;
use crate::simulation::worldgen_of;
use crate::{save_and_load, World, COLD_POOL_BYTES_KEPT};
use protocol::server_parameters::{CleanServerParameters, CAMERA_FORCES_SUPERCHUNKS_HOT, COLD_POOL_BYTES, FORCED_HOT, SEED, SIDE, THREADS};
use protocol::type_registry::FrozenTypeRegistry;
use protocol::world_types::{self, RegisterTypes, TYPES_FILE};
use std::hash::{BuildHasher, RandomState};
use std::path::Path;
use std::sync::Arc;
use utilities::coordinates::WORLD_MIDDLE;

/// The folders every world has, in its own: its superchunks' images and
/// states, its mods, and each mod's config.
pub const WORLD_FOLDERS: [&str; 3] = ["superchunks", "mods", "mod_config"];

/// The world's folder of its mods' config: a folder a mod, its files
/// the mod's own.
pub const MOD_CONFIG: &str = "mod_config";

/// Why a world was not created, set up or run.
#[derive(Debug)]
pub enum WorldNotMade {
    /// A file of it could not be read or written: which, and why.
    Disk(DiskError),
    /// What was asked cannot be -- a number out of range, forced hot
    /// with no side, a world there already, one not set up -- or its
    /// types cannot run a world, or its generation's config is unreadable.
    Refused(String),
}

impl std::fmt::Display for WorldNotMade {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Disk(disk_error) => write!(formatter, "{disk_error}"),
            Self::Refused(why_not) => write!(formatter, "{why_not}"),
        }
    }
}

impl From<DiskError> for WorldNotMade {
    /// The disk's refusal, as a world's.
    fn from(disk_error: DiskError) -> Self {
        Self::Disk(disk_error)
    }
}

/// Creates a world in `folder` as `parameters` say: the folder, made if
/// it is not there, and the folders every world has ([`WORLD_FOLDERS`]);
/// and its world file -- its seed, the one given or else one drawn at
/// random, its side, forced hot or not, its camera. Nothing generated,
/// no types yet. Or why not: a
/// world there already, a number out of range, forced hot with no side,
/// the disk.
pub fn create_world(folder: &Path, parameters: &CleanServerParameters) -> Result<(), WorldNotMade> {
    if folder.join(WORLD_FILE).exists() {
        return Err(WorldNotMade::Refused(format!("{}: a world is there already", folder.display())));
    }
    let side_superchunks = u32::try_from(parameters.integer(SIDE).unwrap_or(0)).map_err(|_| WorldNotMade::Refused(format!("--{SIDE}: a count of superchunks")))?;
    let forced_hot = parameters.nested(FORCED_HOT).is_some();
    if forced_hot && side_superchunks == 0 {
        return Err(WorldNotMade::Refused("only a world with a side can be forced hot".to_string()));
    }
    for world_folder in WORLD_FOLDERS {
        let made_folder = folder.join(world_folder);
        std::fs::create_dir_all(&made_folder).map_err(|io_error| DiskError::IoFailed(made_folder, io_error))?;
    }
    let camera_forces_superchunks_hot = parameters.nested(CAMERA_FORCES_SUPERCHUNKS_HOT).is_some() && !forced_hot;
    let seed = parameters.hex(SEED).unwrap_or_else(|| RandomState::new().hash_one(std::time::SystemTime::now()));
    let world_info = WorldInfo { seed, tick: 0, layers: Vec::new(), side_in_superchunks: Some(side_superchunks).filter(|&side| side > 0), forced_hot, camera_forces_superchunks_hot };
    disk::write_world(folder, &world_info)?;
    Ok(())
}

/// Sets up the world created in `folder` with `frozen_type_registry`, its
/// types made for it -- a new world's, each given the next ID
/// ([`world_types::of_a_new_world`]): kept in its types file. No library
/// of the registry is loaded, nor any code of a type run: that is for
/// running the world. Or why not: no world created there, set up
/// already, the disk.
pub fn set_up_world(folder: &Path, frozen_type_registry: &FrozenTypeRegistry) -> Result<(), WorldNotMade> {
    disk::read_world(folder)?;
    if folder.join(TYPES_FILE).exists() {
        return Err(WorldNotMade::Refused(format!("{}: set up already", folder.display())));
    }
    save_and_load::write_types(folder, frozen_type_registry)?;
    Ok(())
}

/// Runs the world in `folder`, of `frozen_type_registry` -- checked
/// against its types file -- on the threads `parameters` say, its cold
/// pool kept within the bytes they say: its generation set up from its
/// config; never run before -- no hot file -- its starting superchunks
/// generated, every one of a world with a size forced hot, else its
/// middle; else every superchunk as saved, those hot hot again before it
/// ticks. Or why not: not set up, a file unreadable, its types not its
/// registry's, its generation's config.
pub fn run_world(folder: &Path, frozen_type_registry: &Arc<FrozenTypeRegistry>, parameters: &CleanServerParameters) -> Result<World, WorldNotMade> {
    let types_path = folder.join(TYPES_FILE);
    let types_text = std::fs::read_to_string(&types_path).map_err(|io_error| DiskError::IoFailed(types_path.clone(), io_error))?;
    frozen_type_registry.agree_with_saved(&types_text).map_err(|what_is_wrong| DiskError::InvalidFile(types_path, what_is_wrong))?;
    let world_info = WorldInfo { layers: frozen_type_registry.layer_types(), ..disk::read_world(folder)? };
    let worldgen = set_up_worldgen(folder, frozen_type_registry)?;
    let thread_count = thread_count_of(parameters)?;
    let bytes_kept = parameters.integer(COLD_POOL_BYTES).map_or(Ok(COLD_POOL_BYTES_KEPT), u64::try_from).map_err(|_| WorldNotMade::Refused(format!("--{COLD_POOL_BYTES}: a count of bytes")))?;
    let mut world = World::empty_world(world_info, folder, worldgen, thread_count, Arc::clone(frozen_type_registry)).map_err(WorldNotMade::Refused)?;
    world.simulation_handle.keep_cold_pool_within(bytes_kept);
    if folder.join(HOT_FILE).exists() {
        save_and_load::load_saved(folder, &mut world)?;
    } else {
        let every_superchunk = world.simulation_handle.halos().hot.all_superchunks();
        let start_superchunks = if every_superchunk.is_empty() { vec![WORLD_MIDDLE] } else { every_superchunk };
        world.simulation_handle.keep_hot(&start_superchunks);
    }
    Ok(world)
}

/// Creates, sets up and runs a world in `folder` as `parameters` say, of
/// `frozen_type_registry`, a new world's ([`create_world`],
/// [`set_up_world`], [`run_world`]): or why not.
pub fn make_world(folder: &Path, parameters: &CleanServerParameters, frozen_type_registry: &Arc<FrozenTypeRegistry>) -> Result<World, WorldNotMade> {
    create_world(folder, parameters)?;
    set_up_world(folder, frozen_type_registry)?;
    run_world(folder, frozen_type_registry, parameters)
}

/// [`make_world`], of the types `register_types` registers for a new
/// world: or why not, the types refused among it.
pub fn make_world_of(folder: &Path, parameters: &CleanServerParameters, register_types: RegisterTypes) -> Result<World, WorldNotMade> {
    let frozen_type_registry = Arc::new(world_types::of_a_new_world(register_types).map_err(WorldNotMade::Refused)?);
    make_world(folder, parameters, &frozen_type_registry)
}

/// Makes the world in `folder` as it was before it first ran -- its
/// superchunks gone, its hot file, its tick back to 0 -- its world file
/// else, its types, its mods and their config as they are: what runs
/// again from its middle, the world remade. Its world run first let go:
/// nothing of it is read from here after. Or why not.
pub(crate) fn unrun_world(folder: &Path) -> Result<(), WorldNotMade> {
    let superchunks_folder = folder.join(WORLD_FOLDERS[0]);
    std::fs::remove_dir_all(&superchunks_folder).map_err(|io_error| DiskError::IoFailed(superchunks_folder.clone(), io_error))?;
    std::fs::create_dir_all(&superchunks_folder).map_err(|io_error| DiskError::IoFailed(superchunks_folder, io_error))?;
    let hot_path = folder.join(HOT_FILE);
    if hot_path.exists() {
        std::fs::remove_file(&hot_path).map_err(|io_error| DiskError::IoFailed(hot_path, io_error))?;
    }
    let world_info = disk::read_world(folder)?;
    disk::write_world(folder, &WorldInfo { tick: 0, ..world_info })?;
    Ok(())
}

/// The world's generation of `frozen_type_registry`, set up from its
/// mod's folder of the mod config of the world in `folder`: or why not.
fn set_up_worldgen(folder: &Path, frozen_type_registry: &FrozenTypeRegistry) -> Result<SetUpWorldgen, WorldNotMade> {
    let (worldgen, mod_name) = worldgen_of(frozen_type_registry).map_err(WorldNotMade::Refused)?;
    (worldgen.set_up)(&folder.join(MOD_CONFIG).join(mod_name)).map_err(WorldNotMade::Refused)
}

/// The threads a world ticks on, as `parameters` say: every one the
/// machine has if none is given, or 0. Or why not.
fn thread_count_of(parameters: &CleanServerParameters) -> Result<Option<usize>, WorldNotMade> {
    let thread_count = usize::try_from(parameters.integer(THREADS).unwrap_or(0)).map_err(|_| WorldNotMade::Refused(format!("--{THREADS}: a count of threads")))?;
    Ok(Some(thread_count).filter(|&threads| threads > 0))
}
