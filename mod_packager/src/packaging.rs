//! A mod's package made ([`make_the_package`]): its manifest written --
//! its premanifest's fields, the dependencies it carries among those it
//! depends on, its library's hash -- and zipped with its library and
//! the zips of the dependencies it carries, as they are; then read back
//! beside its dependencies' zips, as the game reads them, with every
//! check.

use crate::dependencies::Dependencies;
use crate::premanifest::Premanifest;
use protocol::manifest_format::write_mod_document;
use protocol::manifests::read_mods;
use protocol::mod_packages::{DEPENDENCIES_FOLDER, MANIFEST_FILE, PACKAGE_EXTENSION};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use utilities::archive::write_archive;
use utilities::dynamic_library::library_file_name;

/// The package of the mod in `mod_folder`, whose premanifest is
/// `premanifest`, library `library` -- none, a container's -- and
/// dependencies `dependencies`, made in its `target/` and read back:
/// its path, or why not.
pub fn make_the_package(mod_folder: &Path, premanifest: &Premanifest, library: Option<&Path>, dependencies: &Dependencies) -> Result<PathBuf, String> {
    let read = |path: &Path| std::fs::read(path).map_err(|io_error| format!("{}: {io_error}", path.display()));
    let library_bytes = library.map(read).transpose()?;
    let hash = library_bytes.as_deref().map(|bytes| format!("0x{:x}", Sha256::digest(bytes)));
    let manifest = write_mod_document(&premanifest.manifest(hash));
    let library_file = library_file_name(&premanifest.document().name);
    let mut carried = Vec::new();
    for carried_dependency in &premanifest.document().carried_dependencies {
        let entry = format!("{DEPENDENCIES_FOLDER}{}{PACKAGE_EXTENSION}", carried_dependency.mod_name);
        carried.push(CarriedPackage { bytes: read(&mod_folder.join(&entry))?, entry });
    }
    let mut entries: Vec<(&str, &[u8])> = vec![(MANIFEST_FILE, manifest.as_bytes())];
    if let Some(bytes) = &library_bytes {
        entries.push((&library_file, bytes));
    }
    entries.extend(carried.iter().map(|carried_package| (carried_package.entry.as_str(), carried_package.bytes.as_slice())));
    let target_folder = mod_folder.join("target");
    let package = target_folder.join(format!("{}{PACKAGE_EXTENSION}", premanifest.document().name));
    std::fs::create_dir_all(&target_folder).and_then(|()| std::fs::write(&package, write_archive(&entries))).map_err(|io_error| format!("{}: {io_error}", package.display()))?;
    let packages: Vec<PathBuf> = std::iter::once(package.clone()).chain(dependencies.packages.iter().cloned()).collect();
    read_mods(&packages).map_err(|refusal| format!("{}, read back: {refusal}", package.display()))?;
    Ok(package)
}

/// A carried dependency's zip, as the package holds it: its entry and its bytes.
struct CarriedPackage {
    /// Its entry in the package: in its `dependencies/`.
    entry: String,
    /// The zip.
    bytes: Vec<u8>,
}
