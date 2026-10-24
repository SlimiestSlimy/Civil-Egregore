//! What both sides share, as protocol does: the registry of bundles'
//! libraries by name, a static the binary sets for mods to read, the
//! manifest, and the loader -- the OS's own calls, declared by hand.

use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// A bundle's library: anything, cast down by whoever knows its type.
pub type Library = Arc<dyn Any + Send + Sync>;

/// The libraries the bundles registered, by name.
#[derive(Default)]
pub struct Registry {
    libraries: HashMap<String, Library>,
}

impl Registry {
    pub fn register(&mut self, name: &str, library: Library) {
        self.libraries.insert(name.to_string(), library);
    }

    /// The library `name`, cast down to `T`; why not, if it is not one.
    pub fn library<T: Any + Send + Sync>(&self, name: &str) -> Result<Arc<T>, String> {
        let library = self.libraries.get(name).ok_or(format!("no library {name}"))?;
        library.clone().downcast::<T>().map_err(|_| format!("{name} is no {}", std::any::type_name::<T>()))
    }
}

/// The world's name, set by the binary, read by mods: one copy of the
/// engine, or a mod's own copy finds it unset.
pub static WORLD_NAME: OnceLock<String> = OnceLock::new();

/// Which side loads a bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Common,
    Server,
    Client,
}

/// A bundle as a mod's manifest declares it.
#[derive(Clone, Debug)]
pub struct DeclaredBundle {
    pub name: String,
    pub side: Side,
    pub library_file: PathBuf,
    pub depends_on: Vec<String>,
}

/// The bundles each mod in `mods_folder` declares, in its folder's
/// `manifest.csv`: rows `bundle, name, side, library file, dependencies...`.
pub fn declared_bundles(mods_folder: &Path) -> Result<Vec<DeclaredBundle>, String> {
    let mut bundles = Vec::new();
    let mut mod_folders: Vec<PathBuf> = std::fs::read_dir(mods_folder).map_err(|error| format!("{}: {error}", mods_folder.display()))?.filter_map(|entry| entry.ok().map(|entry| entry.path())).collect();
    mod_folders.sort();
    for mod_folder in mod_folders {
        let manifest = std::fs::read_to_string(mod_folder.join("manifest.csv")).map_err(|error| format!("{}: {error}", mod_folder.display()))?;
        for row in manifest.lines().filter(|row| !row.trim().is_empty()) {
            let cells: Vec<&str> = row.split(',').map(str::trim).collect();
            let [_bundle, name, side, library_file, depends_on @ ..] = cells.as_slice() else {
                return Err(format!("row amiss: {row}"));
            };
            let side = match *side {
                "common" => Side::Common,
                "server" => Side::Server,
                "client" => Side::Client,
                other => return Err(format!("no side {other}")),
            };
            bundles.push(DeclaredBundle { name: name.to_string(), side, library_file: mod_folder.join(library_file), depends_on: depends_on.iter().map(|name| name.to_string()).collect() });
        }
    }
    Ok(bundles)
}

/// `bundles` of the sides in `sides`, in loading order: dependency depth
/// first, then name -- Factorio's rule. A dependency on a bundle of
/// another side counts if that side is loaded too.
pub fn in_loading_order(bundles: &[DeclaredBundle], sides: &[Side]) -> Result<Vec<DeclaredBundle>, String> {
    let loaded: Vec<&DeclaredBundle> = bundles.iter().filter(|bundle| sides.contains(&bundle.side)).collect();
    fn depth(bundle: &DeclaredBundle, all: &[&DeclaredBundle], seen: &mut Vec<String>) -> Result<usize, String> {
        if seen.contains(&bundle.name) {
            return Err(format!("{} depends on itself", bundle.name));
        }
        seen.push(bundle.name.clone());
        let mut deepest = 0;
        for name in &bundle.depends_on {
            let dependency = all.iter().find(|other| &other.name == name).ok_or(format!("{} needs {name}, not loaded", bundle.name))?;
            deepest = deepest.max(1 + depth(dependency, all, seen)?);
        }
        seen.pop();
        Ok(deepest)
    }
    let mut ordered = Vec::new();
    for bundle in &loaded {
        ordered.push((depth(bundle, &loaded, &mut Vec::new())?, (*bundle).clone()));
    }
    ordered.sort_by(|(depth_a, a), (depth_b, b)| depth_a.cmp(depth_b).then(a.name.cmp(&b.name)));
    Ok(ordered.into_iter().map(|(_, bundle)| bundle).collect())
}

/// An opened shared library, never closed.
pub struct OpenedLibrary(*mut std::ffi::c_void);

/// What every bundle has: its registration, given the registry open.
pub const REGISTER_SYMBOL: &str = "bundle_register";
/// What a server or client bundle has besides: its start, given the
/// frozen registry and its side's handle.
pub const START_SYMBOL: &str = "bundle_start";

impl OpenedLibrary {
    pub fn open(path: &Path) -> Result<OpenedLibrary, String> {
        os::open(path).map(OpenedLibrary)
    }

    /// The address of `symbol`, to be cast to the function the bundle's
    /// side says it is.
    pub fn symbol(&self, symbol: &str) -> Result<*mut std::ffi::c_void, String> {
        os::symbol(self.0, symbol)
    }
}

#[cfg(unix)]
mod os {
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    const RTLD_NOW: c_int = 2;

    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
    }

    fn last_error() -> String {
        // SAFETY: dlerror's string lives until the next dl call on this thread.
        unsafe {
            let error = dlerror();
            if error.is_null() { "unknown".to_string() } else { CStr::from_ptr(error).to_string_lossy().into_owned() }
        }
    }

    pub fn open(path: &Path) -> Result<*mut c_void, String> {
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|error| error.to_string())?;
        // SAFETY: a library's constructors run here; the mods are trusted.
        let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW) };
        if handle.is_null() { Err(last_error()) } else { Ok(handle) }
    }

    pub fn symbol(handle: *mut c_void, symbol: &str) -> Result<*mut c_void, String> {
        let symbol = CString::new(symbol).map_err(|error| error.to_string())?;
        // SAFETY: handle came from dlopen and is never closed.
        let address = unsafe { dlsym(handle, symbol.as_ptr()) };
        if address.is_null() { Err(last_error()) } else { Ok(address) }
    }
}

#[cfg(windows)]
mod os {
    use std::ffi::{c_char, c_void, CString};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(file_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    pub fn open(path: &Path) -> Result<*mut c_void, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: a library's DllMain runs here; the mods are trusted.
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() { Err(format!("LoadLibraryW failed: error {}", unsafe { GetLastError() })) } else { Ok(module) }
    }

    pub fn symbol(module: *mut c_void, symbol: &str) -> Result<*mut c_void, String> {
        let symbol = CString::new(symbol).map_err(|error| error.to_string())?;
        // SAFETY: module came from LoadLibraryW and is never freed.
        let address = unsafe { GetProcAddress(module, symbol.as_ptr()) };
        if address.is_null() { Err(format!("GetProcAddress failed: error {}", unsafe { GetLastError() })) } else { Ok(address) }
    }
}

/// A bundle's registration.
pub type Register = fn(&mut Registry);

/// `bundles` opened and registered into `registry`, in order: each with
/// its library, kept open, for its start.
pub fn register_bundles(bundles: &[DeclaredBundle], registry: &mut Registry) -> Result<Vec<(DeclaredBundle, OpenedLibrary)>, String> {
    let mut opened = Vec::new();
    for bundle in bundles {
        let library = OpenedLibrary::open(&bundle.library_file)?;
        // SAFETY: a bundle's registration is a Register, the same toolchain's.
        let register: Register = unsafe { std::mem::transmute(library.symbol(REGISTER_SYMBOL)?) };
        register(registry);
        opened.push((bundle.clone(), library));
    }
    Ok(opened)
}

/// Whether this process has mapped a file whose name holds `part` (Linux).
pub fn mapped(part: &str) -> bool {
    std::fs::read_to_string("/proc/self/maps").map(|maps| maps.lines().any(|line| line.contains(part))).unwrap_or(false)
}
