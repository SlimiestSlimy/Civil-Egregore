//! A world saved in its folder and loaded as it runs again from it
//! ([`save`], [`load_saved`]): every superchunk, hot and cold, and the
//! types it was made with -- loaded with it, the world's own, each the
//! type of its name the code registers.

use crate::World;
use crate::chunk_storage::disk::{self, DiskError, HotSuperchunks, WorldInfo};
use utilities::coordinates::SuperchunkIndex;
use crate::entity_manager::saved;
use crate::simulation::SavedImage;
use std::path::Path;
use std::sync::Arc;
use protocol::type_registry::FrozenTypeRegistry;
use protocol::world_types::{self, RegisterTypes, TYPES_FILE};

/// What a save wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveWritten {
    /// Superchunks written, hot and cold.
    pub superchunks: usize,
    /// Entities written.
    pub entities_written: usize,
    /// Bytes written, every file.
    pub bytes_written: u64,
}

/// The types the world in `folder` was made with, frozen: `register_types`
/// registered, each given the ID its types file
/// ([`TYPES_FILE`]) gives its name -- the first step of loading it,
/// before anything of the world is read.
pub fn read_types(folder: &Path, register_types: RegisterTypes) -> Result<FrozenTypeRegistry, DiskError> {
    let path = folder.join(TYPES_FILE);
    let types_text = std::fs::read_to_string(&path).map_err(|io_error| DiskError::IoFailed(path.clone(), io_error))?;
    world_types::of_a_saved_world(&types_text, register_types).map_err(|what_is_wrong| DiskError::InvalidFile(path, what_is_wrong))
}

/// Writes `frozen_type_registry` as `folder`'s types file: how many bytes.
pub fn write_types(folder: &Path, frozen_type_registry: &FrozenTypeRegistry) -> Result<u64, DiskError> {
    let (path, types_text) = (folder.join(TYPES_FILE), frozen_type_registry.to_text());
    std::fs::write(&path, &types_text).map_err(|io_error| DiskError::IoFailed(path, io_error))?;
    Ok(types_text.len() as u64)
}

/// Saves `world` in its folder, between two ticks.
/// Every dirty bitmap is written back and the ring flushed first, so
/// the cold pool's images are the world's cells; each superchunk's
/// state is its live one if hot, as kept if cold; and which superchunks
/// are hot, which of them cooling, and which warming, in the hot file.
pub fn save(world: &mut World) -> Result<SaveWritten, DiskError> {
    let folder = world.folder.as_path();
    world.simulation_handle.ready_for_save();
    let simulation_handle = &world.simulation_handle;
    let mut save_written = SaveWritten::default();
    let random_states: Vec<(SuperchunkIndex, u64)> = simulation_handle.world_simulation().random_states().collect();
    for superchunk in simulation_handle.chunk_storage().superchunks() {
        save_written.bytes_written += simulation_handle.chunk_storage().save_image(folder, superchunk)?;
        let (words, count) = match simulation_handle.cold().get(&superchunk) {
            Some(words) => (words.clone(), saved::entity_count(words, simulation_handle.entity_store().default_attributes()).expect("a cold superchunk's state, as it was kept")),
            None => {
                let random_state = random_states.binary_search_by_key(&superchunk, |stream_entry| stream_entry.0).ok().map(|found_entry_index| random_states[found_entry_index].1);
                saved::encode_state(random_state, simulation_handle.entity_store().superchunk(superchunk))
            }
        };
        save_written.bytes_written += disk::write_state(folder, superchunk, &words)?;
        save_written.superchunks += 1;
        save_written.entities_written += count;
    }
    save_written.bytes_written += disk::write_hot(folder, &HotSuperchunks { hot: simulation_handle.bitmap_arena().superchunk_indices(), cooling: simulation_handle.cooling().collect(), warming: simulation_handle.warming().collect() })?;
    save_written.bytes_written += write_types(folder, &world.frozen_type_registry)?;
    // The world's file last: a save cut short leaves the one before it.
    let world_info = WorldInfo { tick: simulation_handle.entity_store().current_tick(), ..world.world_info.clone() };
    save_written.bytes_written += disk::write_world(folder, &world_info)?;
    Ok(save_written)
}

/// The worlds saved in `folder`, by their folders' names, sorted:
/// each of its folders a world's file is read from. None if `folder`
/// is not there.
pub fn worlds_in(folder: &Path) -> Vec<String> {
    let Ok(folders) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut worlds: Vec<String> = folders.flatten().filter(|dir_entry| disk::read_world(&dir_entry.path()).is_ok()).map(|world_entry| world_entry.file_name().to_string_lossy().into_owned()).collect();
    worlds.sort_unstable();
    worlds
}

/// Loads into `world`, run from `folder` and empty yet, the world saved
/// there ([`crate::run_world`], of a world that has run): every
/// superchunk's image into the cold pool and its state kept as a cold
/// one's; then the superchunks hot when it was saved made hot, before it
/// ticks, those cooling cooling again and those warming warming again,
/// each to turn when it was to -- as it was when saved, to the cell and
/// the random number. Every image is read and checked, and those past
/// the bytes the world keeps in memory are left in `folder`, read again
/// when wanted -- so the folder is not to be removed while the world
/// runs. Or what is wrong with a file of it.
pub(crate) fn load_saved(folder: &Path, world: &mut World) -> Result<(), DiskError> {
    let hot = disk::read_hot(folder)?;
    let simulation_handle = &mut world.simulation_handle;
    let (mut entities_kept, saved_in) = (0, Arc::<Path>::from(folder));
    let default_attributes = simulation_handle.entity_store().default_attributes().clone();
    for superchunk in disk::saved_superchunks(folder)? {
        // Every image read, and so checked: kept in memory while there is room.
        let image = disk::read_image(folder, superchunk)?;
        let saved_image = if simulation_handle.chunk_storage().over_memory_kept() { SavedImage::InFolder(Arc::clone(&saved_in)) } else { SavedImage::InMemory(image) };
        let (words, path) = disk::read_state(folder, superchunk)?;
        // Every state read whole now: it is decoded only when its superchunk turns hot.
        entities_kept += saved::entity_count(&words, &default_attributes).map_err(|count_error| DiskError::InvalidFile(path, count_error.to_string()))?;
        simulation_handle.put_saved_superchunk(superchunk, saved_image, words);
    }
    simulation_handle.keep_hot(&hot.hot);
    simulation_handle.restore_cooling(hot.cooling);
    for (superchunk, due_tick) in hot.warming {
        simulation_handle.start_warming(superchunk, due_tick);
    }
    let entities_held = simulation_handle.entity_store().len() + simulation_handle.cold().values().map(|words| saved::entity_count(words, &default_attributes).expect("read whole")).sum::<usize>();
    if entities_held != entities_kept {
        return Err(DiskError::InvalidFile(folder.to_path_buf(), format!("{entities_held} of {entities_kept} entities put back: some on a cell taken")));
    }
    Ok(())
}
