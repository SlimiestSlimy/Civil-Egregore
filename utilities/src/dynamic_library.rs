//! A dynamic library opened at runtime, and its functions found by name
//! ([`DynamicLibrary`]): what std has no way to do, asked of the
//! system's loader through `libloading` -- the one part of the game that
//! knows its OS.
//!
//! A library opened is never closed: code of it may be running, or held
//! as a pointer, anywhere until the process ends.

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::Path;

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

    /// The function the library exports as `name`, as `F` -- a function
    /// pointer's type -- or what the loader said.
    ///
    /// # Safety
    ///
    /// `F` must be a function pointer's type, and the very one the library
    /// defines `name` as: built by the same compiler, the same ABI.
    pub unsafe fn function<F: Copy>(&self, name: &str) -> Result<F, String> {
        const { assert!(size_of::<F>() == size_of::<*mut c_void>(), "a function pointer's type") };
        // SAFETY: the caller's word that `F` is what `name` is.
        let symbol = unsafe { self.library.get::<F>(name.as_bytes()) }.map_err(|why| format!("`{name}`: {why}"))?;
        Ok(*symbol)
    }
}
