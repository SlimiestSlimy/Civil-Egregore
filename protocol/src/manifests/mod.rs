//! The mods' manifests, read and checked: each mod's `manifest.cdnf`
//! read as CDNF, checked as a manifest -- its shape, every field, every
//! name, each library's hash -- and then all of them together: every
//! mod's name once, every dependency there and none in a ring, every
//! type named where its bundle may name it. What comes out ([`Manifest`])
//! only this module makes, so the type registry made of them checks
//! nothing. A type is of a kind, said as the registry says it
//! ([`crate::kinds`]).

use crate::kinds::{BundleFields, Hook, Kind};
use std::path::{Path, PathBuf};

mod across_mods;
mod reading;
pub use reading::MANIFEST_FILE;

/// The manifests of the mods in `mod_folders`, each read and checked
/// alone, then all checked together: or the first thing wrong. In the
/// order given.
pub fn read_mods(mod_folders: &[PathBuf]) -> Result<Vec<Manifest>, ManifestError> {
    let manifests = mod_folders.iter().map(|mod_folder| reading::read_manifest(mod_folder)).collect::<Result<Vec<_>, _>>()?;
    across_mods::check_across_mods(&manifests)?;
    Ok(manifests)
}

/// A mod's manifest, checked, with every other mod's: its name and its
/// bundles, in the order written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The mod's name.
    name: String,
    /// The mod's folder, its manifest at its root.
    folder: PathBuf,
    /// Its bundles.
    bundles: Vec<Bundle>,
}

impl Manifest {
    /// The mod's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The mod's folder.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Its bundles, in the order written.
    pub fn bundles(&self) -> &[Bundle] {
        &self.bundles
    }
}

/// A bundle: its name -- no other bundle's of its mod -- its fields, and
/// its types, in the order written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    /// Its name, in its mod.
    name: String,
    /// Its fields: its side, its library and that library's hash, and
    /// the bundles it depends on.
    fields: BundleFields,
    /// Its types.
    types: Vec<Type>,
}

impl Bundle {
    /// Its name, in its mod.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its fields.
    pub fn fields(&self) -> &BundleFields {
        &self.fields
    }

    /// Its types, in the order written.
    pub fn types(&self) -> &[Type] {
        &self.types
    }
}

/// A type: its name -- no other type's in its bundle -- its kind, and
/// its hooks, each one its kind takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    /// Its name, in its bundle.
    name: String,
    /// Its kind, and the kind's fields.
    kind: Kind,
    /// Its hooks.
    hooks: Vec<Hook>,
}

impl Type {
    /// Its name, in its bundle.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its kind, and the kind's fields.
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Its hooks.
    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }
}

/// Why the mods' manifests were refused: the mod, where in its manifest
/// -- its line, if the CDNF said, and the section -- and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    /// The mod's folder.
    pub mod_folder: PathBuf,
    /// The line, if known.
    pub line: Option<usize>,
    /// The section: the bundle, and the type, as far as known.
    pub section: String,
    /// What is wrong.
    pub what: String,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", self.mod_folder.display())?;
        if let Some(line) = self.line {
            write!(formatter, ", line {line}")?;
        }
        if !self.section.is_empty() {
            write!(formatter, ", {}", self.section)?;
        }
        write!(formatter, ": {}", self.what)
    }
}

impl std::error::Error for ManifestError {}
