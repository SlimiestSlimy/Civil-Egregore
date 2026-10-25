//! The cold pool paged to disk: past the bytes it may keep in memory,
//! the cold superchunks of superchunks nobody holds are written to a folder and
//! let go, read back when wanted (`docs/chunk_storage.md`, "Paged to
//! disk").

use crate::cold_storage::world_folder_files::{self, DiskError};
use crate::cold_storage::{ColdStorage, ColdSuperchunk};
use utilities::coordinates::SuperchunkIndex;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Where the cold pool keeps a cold superchunk -- and how it is handed
/// to another thread: in memory, shared, or in a folder to read it from.
#[derive(Clone)]
pub enum ColdSuperchunkKept {
    /// In memory, shared.
    InMemory(Arc<ColdSuperchunk>),
    /// In its file of a folder -- the paging folder, or the world folder
    /// it was loaded from -- as [`world_folder_files::write_cold_superchunk`]
    /// writes it.
    OnDisk(Arc<Path>),
}

impl ColdSuperchunkKept {
    /// The cold superchunk: read now, if it is on disk ([`read_paged_cold_superchunk`]).
    pub fn cold_superchunk(&self, superchunk: SuperchunkIndex) -> Arc<ColdSuperchunk> {
        match self {
            Self::InMemory(cold_superchunk) => Arc::clone(cold_superchunk),
            Self::OnDisk(folder) => Arc::new(read_paged_cold_superchunk(folder, superchunk)),
        }
    }
}

/// The cold superchunk of `superchunk` paged to `folder`, read back. The world
/// cannot go on without it: a file gone or spoilt is a panic that
/// says which.
pub fn read_paged_cold_superchunk(folder: &Path, superchunk: SuperchunkIndex) -> ColdSuperchunk {
    world_folder_files::read_cold_superchunk(folder, superchunk).unwrap_or_else(|disk_error| panic!("a superchunk's cold superchunk paged to disk did not read back: {disk_error}"))
}

/// Bytes `cold_superchunk` takes in memory.
pub(crate) fn bytes_of(cold_superchunk: &ColdSuperchunk) -> u64 {
    std::mem::size_of_val(cold_superchunk.words()) as u64
}

/// The paging folder, in the world's folder.
const PAGING: &str = "paging";

/// A running world's paging folder, `paging/` in its folder, for the cold
/// superchunks it pages out: emptied when made -- what a world that did
/// not stop cleanly left -- and removed when the world stops.
pub(crate) struct PagingFolder {
    /// The folder.
    folder: Arc<Path>,
}

impl PagingFolder {
    /// The paging folder of the world in `world_folder`, made empty.
    fn new_in(world_folder: &Path) -> Result<Self, DiskError> {
        let folder = world_folder.join(PAGING);
        let io_failed = |io_error| DiskError::IoFailed(folder.clone(), io_error);
        match fs::remove_dir_all(&folder) {
            Err(io_error) if io_error.kind() != std::io::ErrorKind::NotFound => return Err(io_failed(io_error)),
            _ => {}
        }
        fs::create_dir_all(&folder).map_err(io_failed)?;
        Ok(Self { folder: folder.into() })
    }
}

impl Drop for PagingFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}

/// How the cold pool is paged.
pub(crate) struct ColdPoolPaging {
    /// Its paging folder, if it pages.
    folder: Option<PagingFolder>,
    /// Bytes of cold superchunks it may keep in memory.
    bytes_kept: u64,
    /// Bytes of cold superchunks in memory now.
    pub(crate) bytes_in_memory: u64,
    /// Where in the pool the next cold superchunk to page out is looked for:
    /// round and round, so each waits as long.
    clock_hand: usize,
}

impl ColdPoolPaging {
    /// Nothing paged: every cold superchunk in memory, however many.
    pub(crate) const fn never_paged() -> Self {
        Self { folder: None, bytes_kept: u64::MAX, bytes_in_memory: 0, clock_hand: 0 }
    }
}

impl ColdStorage {
    /// Has the cold pool keep `bytes_kept` bytes of cold superchunks in memory
    /// at most, the rest paged to the paging folder of the world in
    /// `world_folder` ([`ColdStorage::page_out`]). Refused, if `world_folder`
    /// holds no world: there is no paging outside a world.
    pub fn page_in_world(&mut self, world_folder: &Path, bytes_kept: u64) -> Result<(), DiskError> {
        let world_file = world_folder.join(world_folder_files::WORLD_FILE);
        if !world_file.is_file() {
            return Err(DiskError::InvalidFile(world_file, "no world here to page in".to_string()));
        }
        self.cold_pool_paging.folder = Some(PagingFolder::new_in(world_folder)?);
        self.cold_pool_paging.bytes_kept = bytes_kept;
        Ok(())
    }

    /// Bytes of cold superchunks the cold pool may keep in memory.
    pub fn keep_in_memory(&mut self, bytes_kept: u64) {
        self.cold_pool_paging.bytes_kept = bytes_kept;
    }

    /// Never pages out again: what is on disk stays there, read back
    /// when wanted.
    pub fn stop_paging_out(&mut self) {
        self.cold_pool_paging.bytes_kept = u64::MAX;
    }

    /// Whether more bytes of cold superchunks are in memory than may be kept:
    /// [`ColdStorage::page_out`] is due.
    pub fn over_memory_kept(&self) -> bool {
        self.cold_pool_paging.folder.is_some() && self.cold_pool_paging.bytes_in_memory > self.cold_pool_paging.bytes_kept
    }

    /// Bytes of cold superchunks in memory.
    #[cfg(test)]
    pub fn bytes_in_memory(&self) -> u64 {
        self.cold_pool_paging.bytes_in_memory
    }

    /// Whether the cold superchunk of `superchunk` is on disk, not in memory.
    pub fn is_on_disk(&self, superchunk: SuperchunkIndex) -> bool {
        self.find_superchunk(superchunk).is_ok_and(|pool_index| matches!(self.cold_pool[pool_index].1, ColdSuperchunkKept::OnDisk(_)))
    }

    /// Superchunks whose cold superchunk is on disk.
    #[cfg(test)]
    pub fn on_disk(&self) -> usize {
        self.cold_pool.iter().filter(|(_, kept)| matches!(kept, ColdSuperchunkKept::OnDisk(_))).count()
    }

    /// Pages cold superchunks out until no more bytes are in memory than may be
    /// kept, or none is left that may go: how many went. One may go if
    /// `is_held` does not say it is held -- hot, or turning hot -- and
    /// none of its changes wait in the ring. Looked for round the
    /// pool from where the last call stopped. A cold superchunk that could not
    /// be written stays in memory, and is the error.
    pub fn page_out(&mut self, is_held: impl Fn(SuperchunkIndex) -> bool) -> Result<usize, DiskError> {
        let (mut pool_entries_passed, mut paged) = (0, 0);
        while self.over_memory_kept() && pool_entries_passed < self.cold_pool.len() {
            let pool_index = self.cold_pool_paging.clock_hand % self.cold_pool.len();
            (self.cold_pool_paging.clock_hand, pool_entries_passed) = (pool_index + 1, pool_entries_passed + 1);
            let (superchunk, ColdSuperchunkKept::InMemory(cold_superchunk)) = &self.cold_pool[pool_index] else {
                continue;
            };
            if is_held(*superchunk) || self.holds_changes(*superchunk) {
                continue;
            }
            let folder = Arc::clone(&self.cold_pool_paging.folder.as_ref().expect("over what it keeps: it pages").folder);
            world_folder_files::write_cold_superchunk(&folder, *superchunk, cold_superchunk)?;
            self.cold_pool_paging.bytes_in_memory -= bytes_of(cold_superchunk);
            self.cold_pool[pool_index].1 = ColdSuperchunkKept::OnDisk(folder);
            paged += 1;
        }
        Ok(paged)
    }

    /// Puts `superchunk` in the cold pool as the cold superchunk in its file of
    /// `folder` -- a world saved there -- left on disk until wanted.
    pub fn insert_on_disk(&mut self, superchunk: SuperchunkIndex, folder: &Arc<Path>) {
        let kept = ColdSuperchunkKept::OnDisk(Arc::clone(folder));
        match self.find_superchunk(superchunk) {
            Ok(pool_index) => {
                if let ColdSuperchunkKept::InMemory(cold_superchunk) = std::mem::replace(&mut self.cold_pool[pool_index].1, kept) {
                    self.cold_pool_paging.bytes_in_memory -= bytes_of(&cold_superchunk);
                }
            }
            Err(insert_index) => self.cold_pool.insert(insert_index, (superchunk, kept)),
        }
    }

    /// Puts `cold_superchunk`, read back from disk for `superchunk`, in memory
    /// again -- unless the pool has one in memory since, which is the
    /// newer.
    pub fn bring_in(&mut self, superchunk: SuperchunkIndex, cold_superchunk: ColdSuperchunk) {
        if self.is_on_disk(superchunk) {
            self.insert(superchunk, cold_superchunk);
        }
    }

    /// The cold superchunk of `superchunk` as the pool has it, if it has one: to
    /// hand to another thread, which reads it if it is on disk.
    pub fn kept_cold_superchunk(&self, superchunk: SuperchunkIndex) -> Option<ColdSuperchunkKept> {
        self.find_superchunk(superchunk).ok().map(|pool_index| self.cold_pool[pool_index].1.clone())
    }

    /// Writes the cold superchunk of `superchunk` as its file of `folder`, a
    /// world's save: how many bytes. One on disk is copied there, file
    /// to file -- or left, if that is the very file it is kept in.
    pub fn save_cold_superchunk(&self, folder: &Path, superchunk: SuperchunkIndex) -> Result<u64, DiskError> {
        match self.kept_cold_superchunk(superchunk).expect("a superchunk of the cold pool") {
            ColdSuperchunkKept::InMemory(cold_superchunk) => world_folder_files::write_cold_superchunk(folder, superchunk, &cold_superchunk),
            ColdSuperchunkKept::OnDisk(from_folder) => world_folder_files::copy_cold_superchunk(&from_folder, folder, superchunk),
        }
    }
}
