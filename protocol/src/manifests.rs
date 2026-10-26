//! The mods' manifests, read and checked: each mod's package read
//! ([`crate::mod_packages`]), its manifest read as CDNF, then as the format
//! every manifest-alike file shares ([`crate::manifest_format`]), then as
//! a manifest -- no carried dependencies, a premanifest's; a library in
//! the package and its hash, for every mod but a container -- the mods
//! it carries read the same, and then all of them together: every
//! dependency there at its version and on a side it may be, none in a
//! ring, every type named where its mod may name it. What comes out
//! ([`Manifest`]) only this module makes, so the type registry made of
//! them checks nothing. A type is of a kind, said as the registry says
//! it ([`crate::type_kinds`]).

use crate::manifest_format::{read_mod_document, FormatError, ModDocument};
use crate::mod_packages::{PackagePath, DEPENDENCIES_FOLDER, MANIFEST_FILE, PACKAGE_EXTENSION};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use utilities::archive::read_archive;
use utilities::dynamic_library::library_file_name;

mod across_mods;

/// The manifests of the mods in the packages `packages` and of every mod
/// they carry, flattened -- each mod before those it carries, a mod read
/// twice kept once if its manifest is the same both times -- each read
/// and checked alone, then all checked together: or the first thing
/// wrong. In the order given.
pub fn read_mods(packages: &[PathBuf]) -> Result<Vec<Manifest>, ManifestError> {
    let mut read_so_far: Vec<ReadManifest> = Vec::new();
    for package in packages {
        let path = PackagePath::on_disk(package);
        let bytes = std::fs::read(package).map_err(|io_error| ManifestError { package: path.clone(), line: None, section: String::new(), what: io_error.to_string() })?;
        read_package(&bytes, path, &mut read_so_far)?;
    }
    let manifests: Vec<Manifest> = read_so_far.into_iter().map(|read| read.manifest).collect();
    across_mods::check_across_mods(&manifests)?;
    Ok(manifests)
}

/// The mod of the package `package`, at `path`, and every mod it
/// carries, each read and checked alone, put in `read_so_far` with its
/// manifest's text: a mod read before kept once, if its manifest is the
/// same.
fn read_package(package: &[u8], path: PackagePath, read_so_far: &mut Vec<ReadManifest>) -> Result<(), ManifestError> {
    let refused = |what: String| ManifestError { package: path.clone(), line: None, section: String::new(), what };
    let entries = read_archive(package).map_err(refused)?;
    let manifest_bytes = entries.iter().find(|&&(entry, _)| entry == MANIFEST_FILE).ok_or_else(|| refused(format!("no {MANIFEST_FILE}")))?.1;
    let text = std::str::from_utf8(manifest_bytes).map_err(|_| refused(format!("{MANIFEST_FILE}: not UTF-8")))?;
    let manifest = read_manifest(text, &entries, &path)?;
    if let Some(read_before) = read_so_far.iter().find(|read_before| read_before.manifest.document.name == manifest.document.name) {
        if read_before.manifest_text != text {
            return Err(refused(format!("a mod named as the one in {} is, its manifest not the same", read_before.manifest.package)));
        }
        return Ok(());
    }
    let library_file = (!manifest.document.is_container).then(|| library_file_name(&manifest.document.name));
    read_so_far.push(ReadManifest { manifest, manifest_text: text.to_string() });
    for &(entry, bytes) in &entries {
        let carried = entry.strip_prefix(DEPENDENCIES_FOLDER).is_some_and(|file| !file.contains('/') && file.ends_with(PACKAGE_EXTENSION));
        if carried {
            read_package(bytes, path.inside(entry), read_so_far)?;
        } else if entry != MANIFEST_FILE && Some(entry) != library_file.as_deref() {
            return Err(refused(format!("{entry}: neither the manifest, the library nor a carried mod's package")));
        }
    }
    Ok(())
}

/// The manifest `text` of the mod whose package, at `path`, has the
/// entries `entries`, read and checked alone.
fn read_manifest(text: &str, entries: &[(&str, &[u8])], path: &PackagePath) -> Result<Manifest, ManifestError> {
    let refused = |line: Option<usize>, section: &str, what: String| ManifestError { package: path.clone(), line, section: section.to_string(), what };
    let document = cdnf::parse(text).map_err(|cdnf_error| refused(Some(cdnf_error.line), "", cdnf_error.what))?;
    let document = read_mod_document(&document).map_err(|FormatError { section, what }| refused(None, &section, what))?;
    let in_mod = |what: String| refused(None, &document.name, what);
    if !document.carried_dependencies.is_empty() {
        return Err(in_mod("carried dependencies: a premanifest's field, the manifest's depends on holds them".to_string()));
    }
    if document.depends_on.iter().any(|dependency| dependency.source.is_some()) {
        return Err(in_mod("a dependency's source: a premanifest's, the packager's to read".to_string()));
    }
    if !document.is_container {
        let hash_said = document.hash.as_deref().ok_or_else(|| in_mod("hash: missing, the library's".to_string()))?;
        let library_file = library_file_name(&document.name);
        let library_bytes = entries.iter().find(|&&(entry, _)| entry == library_file).ok_or_else(|| in_mod(format!("no library {library_file}")))?.1;
        let hash = format!("0x{:x}", Sha256::digest(library_bytes));
        if !hash.eq_ignore_ascii_case(hash_said) {
            return Err(in_mod(format!("library {library_file}: its hash is {hash}, the manifest says {hash_said}")));
        }
    }
    Ok(Manifest { document, package: path.clone() })
}

/// A manifest read, and its text: a mod read twice is kept once if
/// its manifest's text is the same both times.
struct ReadManifest {
    /// The manifest.
    manifest: Manifest,
    /// Its text.
    manifest_text: String,
}

/// A mod's manifest, checked with every other mod's: the mod as its
/// manifest says it -- a hash unless it is a container, no carried
/// dependencies -- and where its package is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The mod as its manifest says it.
    document: ModDocument,
    /// Where the mod's package is.
    package: PackagePath,
}

impl Manifest {
    /// The mod as its manifest says it.
    pub fn document(&self) -> &ModDocument {
        &self.document
    }

    /// Where the mod's package is.
    pub fn package(&self) -> &PackagePath {
        &self.package
    }
}

/// Why the mods' manifests were refused: the mod's package, where in its
/// manifest -- its line, if the CDNF said, and the section -- and what is
/// wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    /// Where the mod's package is.
    pub package: PackagePath,
    /// The line, if known.
    pub line: Option<usize>,
    /// The section: the mod, and the type, as far as known.
    pub section: String,
    /// What is wrong.
    pub what: String,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "{}", self.package)?;
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
