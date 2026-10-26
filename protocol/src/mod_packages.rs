//! A mod's package: the zip it is shipped as -- its manifest
//! ([`MANIFEST_FILE`]), its library if it is no container, and the
//! packages of the mods it carries, each a zip in it
//! ([`DEPENDENCIES_FOLDER`]) -- and where a mod's package is, on the disk
//! or inside another's ([`PackagePath`]).

use std::path::{Path, PathBuf};
use utilities::archive::read_archive;

/// A mod's manifest, at the root of its package.
pub const MANIFEST_FILE: &str = "manifest.cdnf";

/// Where a package keeps the packages of the mods it carries.
pub const DEPENDENCIES_FOLDER: &str = "dependencies/";

/// A package's file's extension.
pub const PACKAGE_EXTENSION: &str = ".zip";

/// Where a mod's package is: a package on the disk, and the packages
/// inside it down to the mod's, each by its path in the one before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagePath {
    /// The package on the disk.
    pub file: PathBuf,
    /// The packages inside it, outermost first: none if the mod's is the
    /// one on the disk.
    pub nested: Vec<String>,
}

impl PackagePath {
    /// The package `file`, on the disk.
    pub fn on_disk(file: &Path) -> Self {
        Self { file: file.to_path_buf(), nested: Vec::new() }
    }

    /// The package at `entry` in this one.
    pub fn inside(&self, entry: &str) -> Self {
        Self { file: self.file.clone(), nested: [&self.nested[..], &[entry.to_string()]].concat() }
    }

    /// The bytes of the package's entry `entry`: read from the disk,
    /// through each package inside down to this one.
    pub fn read_entry(&self, entry: &str) -> Result<Vec<u8>, String> {
        let file = std::fs::read(&self.file).map_err(|io_error| format!("{}: {io_error}", self.file.display()))?;
        let mut bytes: &[u8] = &file;
        for each_entry in self.nested.iter().map(String::as_str).chain([entry]) {
            let entries = read_archive(bytes).map_err(|why| format!("{self}: {why}"))?;
            bytes = entries.into_iter().find(|&(entry_name, _)| entry_name == each_entry).ok_or_else(|| format!("{self}: no {each_entry}"))?.1;
        }
        Ok(bytes.to_vec())
    }
}

impl std::fmt::Display for PackagePath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", self.file.display())?;
        for entry in &self.nested {
            write!(formatter, " > {entry}")?;
        }
        Ok(())
    }
}
