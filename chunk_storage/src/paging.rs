//! The cold pool paged to disk: past the bytes it may keep in memory,
//! the images of superchunks nobody holds are written to a folder and
//! let go, read back when wanted (`docs/chunk_storage.md`, "Paged to
//! disk").

use crate::disk::{self, DiskError};
use crate::{ChunkStorage, SuperchunkImage};
use coordinates::SuperchunkIndex;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Where the cold pool keeps a superchunk's image.
pub(crate) enum Kept {
    /// In memory, shared (`docs/chunk_storage.md`, "Shared images").
    InMemory(Arc<SuperchunkImage>),
    /// In its file of this world's folder -- the paging folder, or the
    /// save it was loaded from -- as [`disk::write_image`] writes it.
    OnDisk(Arc<Path>),
}

/// A superchunk's image as the cold pool has it, to hand to another
/// thread: the image itself, or where to read it.
pub enum Stored {
    /// The image, shared.
    InMemory(Arc<SuperchunkImage>),
    /// The folder its file is in ([`disk::read_image`]).
    OnDisk(Arc<Path>),
}

impl Stored {
    /// The image: read now, if it is on disk ([`read_back`]).
    pub fn image(&self, superchunk: SuperchunkIndex) -> Arc<SuperchunkImage> {
        match self {
            Self::InMemory(image) => Arc::clone(image),
            Self::OnDisk(folder) => Arc::new(read_back(folder, superchunk)),
        }
    }
}

/// The image of `superchunk` paged to `folder`, read back. The world
/// cannot go on without it: a file gone or spoilt is a panic that
/// says which.
pub fn read_back(folder: &Path, superchunk: SuperchunkIndex) -> SuperchunkImage {
    disk::read_image(folder, superchunk).unwrap_or_else(|error| panic!("a superchunk's image paged to disk did not read back: {error}"))
}

/// Bytes `image` takes in memory.
pub(crate) fn bytes_of(image: &SuperchunkImage) -> u64 {
    std::mem::size_of_val(image.words()) as u64
}

/// What ends the name of a paging folder's lock file, beside it.
const LOCK: &str = "lock";

/// Counts the paging folders this process has made: each a name of
/// its own.
static FOLDERS_MADE: AtomicU64 = AtomicU64::new(0);

/// A folder of a running world's own, for the images it pages out:
/// gone with the world. Beside it a lock file its process holds
/// locked, so a folder a process left behind by dying is known by its
/// lock being free, and removed by the next to make one.
pub(crate) struct PagingFolder {
    /// The folder.
    folder: Arc<Path>,
    /// Its lock file, and that file held open and locked.
    lock: (PathBuf, File),
}

impl PagingFolder {
    /// A new folder under `root`, those left behind there removed.
    fn under(root: &Path) -> Result<Self, DiskError> {
        let io = |path: &Path| { let path = path.to_path_buf(); move |error| DiskError::Io(path, error) };
        fs::create_dir_all(root).map_err(io(root))?;
        for entry in fs::read_dir(root).map_err(io(root))?.flatten() {
            let lock = entry.path();
            // A lock nobody holds: its world is gone. One held, or just taken by a process starting, is left.
            if lock.extension().is_some_and(|extension| extension == LOCK) && File::open(&lock).is_ok_and(|file| file.try_lock().is_ok()) {
                let _ = fs::remove_dir_all(lock.with_extension(""));
                let _ = fs::remove_file(&lock);
            }
        }
        loop {
            let folder = root.join(format!("{}_{}", std::process::id(), FOLDERS_MADE.fetch_add(1, Ordering::Relaxed)));
            let lock = folder.with_extension(LOCK);
            let file = File::create(&lock).map_err(io(&lock))?;
            // Taken by another process sweeping at this very moment: another name.
            if file.try_lock().is_err() {
                continue;
            }
            // A dead process of the same number may have left one.
            let _ = fs::remove_dir_all(&folder);
            fs::create_dir_all(&folder).map_err(io(&folder))?;
            return Ok(Self { folder: folder.into(), lock: (lock, file) });
        }
    }
}

impl Drop for PagingFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
        let _ = self.lock.1.unlock();
        let _ = fs::remove_file(&self.lock.0);
    }
}

/// How the cold pool is paged.
pub(crate) struct Paging {
    /// Under which folder its paging folder is made, if it pages.
    root: Option<PathBuf>,
    /// Its paging folder, made when the first image is paged out.
    folder: Option<PagingFolder>,
    /// Bytes of images it may keep in memory.
    bytes_kept: u64,
    /// Bytes of images in memory now.
    pub(crate) bytes_in_memory: u64,
    /// Where in the pool the next image to page out is looked for:
    /// round and round, so each waits as long.
    hand: usize,
}

impl Paging {
    /// Nothing paged: every image in memory, however many.
    pub(crate) const fn none() -> Self {
        Self { root: None, folder: None, bytes_kept: u64::MAX, bytes_in_memory: 0, hand: 0 }
    }
}

impl ChunkStorage {
    /// Has the cold pool keep `bytes_kept` bytes of images in memory
    /// at most, the rest paged to a folder of its own made under
    /// `root` ([`ChunkStorage::page_out`]).
    pub fn page_under(&mut self, root: PathBuf, bytes_kept: u64) {
        self.paging.root = Some(root);
        self.paging.bytes_kept = bytes_kept;
    }

    /// Bytes of images the cold pool may keep in memory.
    pub fn keep_in_memory(&mut self, bytes_kept: u64) {
        self.paging.bytes_kept = bytes_kept;
    }

    /// Never pages out again: what is on disk stays there, read back
    /// when wanted.
    pub fn stop_paging_out(&mut self) {
        self.paging.bytes_kept = u64::MAX;
    }

    /// Whether more bytes of images are in memory than may be kept:
    /// [`ChunkStorage::page_out`] is due.
    pub fn over_memory_kept(&self) -> bool {
        self.paging.root.is_some() && self.paging.bytes_in_memory > self.paging.bytes_kept
    }

    /// Bytes of images in memory.
    pub fn bytes_in_memory(&self) -> u64 {
        self.paging.bytes_in_memory
    }

    /// Whether the image of `superchunk` is on disk, not in memory.
    pub fn is_on_disk(&self, superchunk: SuperchunkIndex) -> bool {
        self.find(superchunk).is_ok_and(|at| matches!(self.cold_pool[at].1, Kept::OnDisk(_)))
    }

    /// Superchunks whose image is on disk.
    pub fn on_disk(&self) -> usize {
        self.cold_pool.iter().filter(|(_, kept)| matches!(kept, Kept::OnDisk(_))).count()
    }

    /// Pages images out until no more bytes are in memory than may be
    /// kept, or none is left that may go: how many went. One may go if
    /// `held` does not say it is held -- hot, or turning hot -- and
    /// none of its changes wait in the ring. Looked for round the
    /// pool from where the last call stopped. An image that could not
    /// be written stays in memory, and is the error.
    pub fn page_out(&mut self, held: impl Fn(SuperchunkIndex) -> bool) -> Result<usize, DiskError> {
        let (mut passed, mut paged) = (0, 0);
        while self.over_memory_kept() && passed < self.cold_pool.len() {
            let at = self.paging.hand % self.cold_pool.len();
            (self.paging.hand, passed) = (at + 1, passed + 1);
            let (superchunk, Kept::InMemory(image)) = &self.cold_pool[at] else {
                continue;
            };
            if held(*superchunk) || self.holds_changes(*superchunk) {
                continue;
            }
            let folder = match &self.paging.folder {
                Some(made) => Arc::clone(&made.folder),
                None => Arc::clone(&self.paging.folder.insert(PagingFolder::under(self.paging.root.as_deref().expect("over what it keeps: it pages"))?).folder),
            };
            disk::write_image(&folder, *superchunk, image)?;
            self.paging.bytes_in_memory -= bytes_of(image);
            self.cold_pool[at].1 = Kept::OnDisk(folder);
            paged += 1;
        }
        Ok(paged)
    }

    /// Puts `superchunk` in the cold pool as the image in its file of
    /// `folder` -- a world saved there -- left on disk until wanted.
    pub fn insert_on_disk(&mut self, superchunk: SuperchunkIndex, folder: &Arc<Path>) {
        let kept = Kept::OnDisk(Arc::clone(folder));
        match self.find(superchunk) {
            Ok(at) => {
                if let Kept::InMemory(image) = std::mem::replace(&mut self.cold_pool[at].1, kept) {
                    self.paging.bytes_in_memory -= bytes_of(&image);
                }
            }
            Err(at) => self.cold_pool.insert(at, (superchunk, kept)),
        }
    }

    /// Puts `image`, read back from disk for `superchunk`, in memory
    /// again -- unless the pool has one in memory since, which is the
    /// newer.
    pub fn bring_in(&mut self, superchunk: SuperchunkIndex, image: SuperchunkImage) {
        if self.is_on_disk(superchunk) {
            self.insert(superchunk, image);
        }
    }

    /// The image of `superchunk` as the pool has it, if it has one: to
    /// hand to another thread, which reads it if it is on disk.
    pub fn stored(&self, superchunk: SuperchunkIndex) -> Option<Stored> {
        self.find(superchunk).ok().map(|at| match &self.cold_pool[at].1 {
            Kept::InMemory(image) => Stored::InMemory(Arc::clone(image)),
            Kept::OnDisk(folder) => Stored::OnDisk(Arc::clone(folder)),
        })
    }

    /// Writes the image of `superchunk` as its file of `folder`, a
    /// world's save: how many bytes. One on disk is copied there, file
    /// to file -- or left, if that is the very file it is kept in.
    pub fn save_image(&self, folder: &Path, superchunk: SuperchunkIndex) -> Result<u64, DiskError> {
        match self.stored(superchunk).expect("a superchunk of the cold pool") {
            Stored::InMemory(image) => disk::write_image(folder, superchunk, &image),
            Stored::OnDisk(from) => disk::copy_image(&from, folder, superchunk),
        }
    }
}
