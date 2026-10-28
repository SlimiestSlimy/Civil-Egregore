//! A dynamic library opened at runtime, and the statics it exports
//! found by name ([`DynamicLibrary`]): what std has no way to do, asked of the
//! system's loader through `libloading` -- the one part of the game that
//! knows its OS.
//!
//! A library opened is never closed: code of it may be running, or held
//! as a pointer, anywhere until the process ends.

use std::mem::ManuallyDrop;
use std::path::Path;

/// The start of the name of a process's folder of libraries opened from
/// bytes, its ID after.
const TEMPORARY_FOLDER_PREFIX: &str = "civil_egregore_libraries_";

/// A dynamic library, open for the rest of the process.
pub struct DynamicLibrary {
    /// The library: never dropped, so never closed.
    library: ManuallyDrop<libloading::Library>,
}

impl DynamicLibrary {
    /// Opens the library at `path`: or what the loader said.
    pub fn open(path: &Path) -> Result<Self, String> {
        // SAFETY: opening runs whatever the library runs as it opens; a
        // mod's library is the one its checked manifest names.
        let library = unsafe { libloading::Library::new(path) }.map_err(|why| format!("{}: {why}", path.display()))?;
        Ok(Self { library: ManuallyDrop::new(library) })
    }

    /// Opens the library of the mod `mod_name` from its bytes, `bytes`:
    /// or what went wrong. The loader opens files alone, so the bytes are
    /// written to one, by the mod's library's file name -- what its
    /// dependents find it by -- in a folder of this process's in the
    /// system's temporary folder, opened, and removed. Where the OS keeps
    /// an open library's file, Windows, it is left -- never another
    /// process's touched -- for the system to clear.
    pub fn from_bytes(mod_name: &str, bytes: &[u8]) -> Result<Self, String> {
        let own_folder = std::env::temp_dir().join(format!("{TEMPORARY_FOLDER_PREFIX}{}", std::process::id()));
        let file = own_folder.join(library_file_name(mod_name));
        std::fs::create_dir_all(&own_folder).and_then(|()| std::fs::write(&file, bytes)).map_err(|io_error| format!("{}: {io_error}", file.display()))?;
        let opened = Self::open(&file);
        _ = std::fs::remove_file(&file);
        _ = std::fs::remove_dir(&own_folder);
        opened
    }

    /// The value of the static the library exports as `name`, as `T`:
    /// or what the loader said.
    ///
    /// # Safety
    ///
    /// `T` must be the very type the library defines `name` as: built by
    /// the same compiler, the same ABI.
    pub unsafe fn static_value<T: Copy>(&self, name: &str) -> Result<T, String> {
        // SAFETY: the caller's word that `T` is what `name` is; a static's
        // symbol is where it is kept.
        let symbol = unsafe { self.library.get::<*const T>(name.as_bytes()) }.map_err(|why| format!("`{name}`: {why}"))?;
        // SAFETY: a static, kept as long as the library, never closed.
        Ok(unsafe { **symbol })
    }
}

/// The file the mod `mod_name`'s library is, on this OS -- as its
/// crate, of the mod's name, built it: `lib<name>.so` on Linux,
/// `<name>.dll` on Windows.
pub fn library_file_name(mod_name: &str) -> String {
    format!("{}{mod_name}{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX)
}
