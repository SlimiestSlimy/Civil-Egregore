//! A mod's dependencies read and checked, and their libraries taken out
//! for the build ([`read_dependencies`]): each a zip in the mod's
//! `dependencies/` folder -- every one its premanifest lists, carried or
//! not, at the version it says, and those they need in turn, none
//! nothing needs -- read as the game reads them, with every check, and
//! every library in them written to one folder rustc finds them in.

use crate::premanifest::Premanifest;
use protocol::manifest_format::DeclaredDependency;
use protocol::manifests::{read_mods, Manifest};
use protocol::mod_packages::{DEPENDENCIES_FOLDER, PACKAGE_EXTENSION};
use std::path::{Path, PathBuf};
use utilities::dynamic_library::library_file_name;

/// Where, in a mod's crate, its dependencies' libraries are taken out
/// to, made anew each time.
pub const EXTRACTED_FOLDER: &str = "target/civil_egregore_dependencies";

/// A mod's dependencies: their zips, and the mods in them.
pub struct Dependencies {
    /// The zips in the mod's `dependencies/`, in order of name.
    pub packages: Vec<PathBuf>,
    /// Every mod in them, carried ones too, read and checked together.
    pub manifests: Vec<Manifest>,
}

/// The dependencies of the mod in `mod_folder`, whose premanifest is
/// `premanifest`, read and checked, their libraries taken out to its
/// [`EXTRACTED_FOLDER`]: or what is wrong.
pub fn read_dependencies(mod_folder: &Path, premanifest: &Premanifest) -> Result<Dependencies, String> {
    let folder = mod_folder.join(DEPENDENCIES_FOLDER);
    let mut named_packages: Vec<NamedPackage> = Vec::new();
    if folder.exists() {
        for entry in std::fs::read_dir(&folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))? {
            let path = entry.map_err(|io_error| format!("{}: {io_error}", folder.display()))?.path();
            let name = path.file_name().and_then(|file_name| file_name.to_str()?.strip_suffix(PACKAGE_EXTENSION));
            match name {
                Some(name) if path.is_file() => named_packages.push(NamedPackage { mod_name: name.to_string(), package: path.clone() }),
                _ => return Err(format!("{}: a dependency's zip alone may be there", path.display())),
            }
        }
    }
    named_packages.sort_by(|one, other| one.mod_name.cmp(&other.mod_name));
    let packages: Vec<PathBuf> = named_packages.iter().map(|named| named.package.clone()).collect();
    let manifests = read_mods(&packages).map_err(|refusal| refusal.to_string())?;
    let listed: Vec<&DeclaredDependency> = premanifest.document().depends_on.iter().chain(&premanifest.document().carried_dependencies).collect();
    for DeclaredDependency { mod_name: name, version, .. } in listed.iter().copied() {
        let Some(manifest) = manifests.iter().find(|manifest| manifest.document().name == *name) else {
            return Err(format!("depends on {name}: no {DEPENDENCIES_FOLDER}{name}{PACKAGE_EXTENSION}"));
        };
        if manifest.document().version != *version {
            return Err(format!("depends on {name} {version}: {name} is {}", manifest.document().version));
        }
    }
    let needed = needed_by(&listed, &manifests);
    for NamedPackage { mod_name: name, package } in &named_packages {
        // Its own mod read as another's, if the same mod was read before it: none then.
        let read_as = manifests.iter().find(|manifest| manifest.package().file == *package && manifest.package().nested.is_empty());
        if read_as.is_some_and(|manifest| manifest.document().name != *name) || !needed.contains(&name.as_str()) {
            return Err(format!("{}: not the zip of a mod the mod needs, by its name", package.display()));
        }
    }
    extract_libraries(&mod_folder.join(EXTRACTED_FOLDER), &manifests)?;
    Ok(Dependencies { packages, manifests })
}

/// A zip in a mod's `dependencies/`, and the mod it is named for.
struct NamedPackage {
    /// The mod's name: the zip's, without its extension.
    mod_name: String,
    /// The zip.
    package: PathBuf,
}

/// The names of the mods `listed` and of every mod they depend on in
/// turn, among `manifests`.
fn needed_by<'mods>(listed: &[&'mods DeclaredDependency], manifests: &'mods [Manifest]) -> Vec<&'mods str> {
    let mut needed: Vec<&str> = listed.iter().map(|declared| declared.mod_name.as_str()).collect();
    let mut next = 0;
    while let Some(&name) = needed.get(next) {
        next += 1;
        for manifest in manifests.iter().filter(|manifest| manifest.document().name == name) {
            for declared in &manifest.document().depends_on {
                if !needed.contains(&declared.mod_name.as_str()) {
                    needed.push(&declared.mod_name);
                }
            }
        }
    }
    needed
}

/// Every library of `manifests` written to `folder`, emptied first.
fn extract_libraries(folder: &Path, manifests: &[Manifest]) -> Result<(), String> {
    if folder.exists() {
        std::fs::remove_dir_all(folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))?;
    }
    std::fs::create_dir_all(folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))?;
    for manifest in manifests.iter().filter(|manifest| !manifest.document().is_container) {
        let library_file = library_file_name(&manifest.document().name);
        let library_bytes = manifest.package().read_entry(&library_file)?;
        std::fs::write(folder.join(&library_file), library_bytes).map_err(|io_error| format!("{}: {io_error}", folder.join(&library_file).display()))?;
    }
    Ok(())
}
