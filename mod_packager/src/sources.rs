//! A mod's dependencies packaged from their sources
//! ([`package_the_sources`]): each its premanifest gives a source --
//! a mod's folder, relative to its own -- packaged as any mod is, first,
//! and its zip copied into the mod's `dependencies/`, where its
//! dependencies are read from as ever. Once a run: a mod packaged
//! before in it is not again; one packaged while it is being packaged
//! is a cycle, refused.

use crate::premanifest::Premanifest;
use protocol::packages::{DEPENDENCIES_FOLDER, PACKAGE_EXTENSION};
use std::path::{Path, PathBuf};

/// What one run of the packager has packaged, and is packaging.
#[derive(Default)]
pub struct PackagingRun {
    /// The mods' folders being packaged, each waiting on the next.
    pub in_progress: Vec<PathBuf>,
    /// The mods packaged: each its folder and its zip.
    pub packaged: Vec<(PathBuf, PathBuf)>,
}

/// The sourced dependencies of the mod in `mod_folder`, whose
/// premanifest is `premanifest`, packaged in `run` -- or taken from it --
/// and copied into its `dependencies/`: or why not.
pub fn package_the_sources(mod_folder: &Path, premanifest: &Premanifest, run: &mut PackagingRun) -> Result<(), String> {
    for (name, source) in &premanifest.document().sources {
        let source_folder = mod_folder.join(source);
        let source_folder = source_folder.canonicalize().map_err(|io_error| format!("{name}'s source, {}: {io_error}", source_folder.display()))?;
        if run.in_progress.contains(&source_folder) {
            return Err(format!("{name}'s source, {}: being packaged already, a cycle", source_folder.display()));
        }
        let package = match run.packaged.iter().find(|(folder, _)| *folder == source_folder) {
            Some((_, package)) => package.clone(),
            None => crate::package_the_mod(&source_folder, run)?,
        };
        if package.file_stem().and_then(|stem| stem.to_str()) != Some(name.as_str()) {
            return Err(format!("{name}'s source, {}: a mod of another name, packaged as {}", source_folder.display(), package.display()));
        }
        let copied = mod_folder.join(format!("{DEPENDENCIES_FOLDER}{name}{PACKAGE_EXTENSION}"));
        std::fs::create_dir_all(mod_folder.join(DEPENDENCIES_FOLDER)).and_then(|()| std::fs::copy(&package, &copied)).map_err(|io_error| format!("{}: {io_error}", copied.display()))?;
    }
    Ok(())
}
