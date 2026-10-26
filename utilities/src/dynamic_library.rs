//! A dynamic library opened at runtime, and its functions found by name
//! ([`DynamicLibrary`]): what std has no way to do, asked of the
//! system's loader. Linux for now -- `dlopen` and `dlsym`, in the C
//! library std links already -- Windows later, behind the same calls.
//!
//! A library opened is never closed: code of it may be running, or held
//! as a pointer, anywhere until the process ends.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

/// Resolve every symbol as the library opens, not as first called: what
/// is missing is said at once.
const RTLD_NOW: c_int = 2;

unsafe extern "C" {
    /// Opens the library at `filename`, with `flags`: a handle, or null.
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    /// The address of the symbol `symbol` in the library of `handle`, or null.
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    /// What the last of these calls to fail said, or null.
    fn dlerror() -> *mut c_char;
}

/// What the loader last said went wrong.
fn loader_error() -> String {
    // SAFETY: dlerror returns null or a string the loader keeps until the next call.
    let said = unsafe { dlerror() };
    if said.is_null() {
        return "the loader said nothing".to_string();
    }
    // SAFETY: not null, so a C string of the loader's.
    unsafe { CStr::from_ptr(said) }.to_string_lossy().into_owned()
}

/// A dynamic library, open for the rest of the process.
pub struct DynamicLibrary {
    /// The loader's handle of it.
    handle: *mut c_void,
}

// SAFETY: the handle is the loader's, which may be asked of from any thread.
unsafe impl Send for DynamicLibrary {}
// SAFETY: as above; nothing of it changes once open.
unsafe impl Sync for DynamicLibrary {}

impl DynamicLibrary {
    /// Opens the library at `path`, every symbol of it resolved: or what
    /// the loader said.
    pub fn open(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        let path_text = CString::new(path.as_os_str().as_bytes()).map_err(|_| format!("{}: a path with a nul in it", path.display()))?;
        // SAFETY: a C string, alive for the call.
        let handle = unsafe { dlopen(path_text.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return Err(format!("{}: {}", path.display(), loader_error()));
        }
        Ok(Self { handle })
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
        let symbol = CString::new(name).map_err(|_| format!("`{name}`: a name with a nul in it"))?;
        // SAFETY: a handle of an open library, and a C string alive for the call.
        let address = unsafe { dlsym(self.handle, symbol.as_ptr()) };
        if address.is_null() {
            return Err(format!("`{name}`: {}", loader_error()));
        }
        // SAFETY: the caller's word that `F` is what `name` is; of a pointer's size, checked above.
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&address) })
    }
}
