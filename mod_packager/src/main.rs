//! The mod packager: a mod -- its crate, or a container's folder --
//! made into the zip it ships as. Its premanifest read and checked; the
//! dependencies it gives a source of packaged first, into its
//! `dependencies/` folder; its dependencies' zips there read and checked;
//! the mod built against the game and them; its set up functions
//! checked; its manifest written; all of it zipped, and read back as the
//! game reads it.
//!
//! `cargo package_mod <mod folder>`, from the game (an alias in its
//! `.cargo/config.toml`). And a mod's crate made, empty but ready to
//! package: `cargo package_mod new <mod folder> <side>`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::path::PathBuf;
use std::process::ExitCode;

mod building;
mod dependencies;
mod new_mod;
mod packaging;
mod premanifest;
mod set_up_check;
mod sources;

/// The mod in the folder given packaged, or a new one made; or why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let done = match arguments.as_slice() {
        [mod_folder] => package_the_mod(&PathBuf::from(mod_folder), &mut sources::PackagingRun::default()).map(|_| ()),
        [new, mod_folder, side] if new == "new" => new_mod::make_a_new_mod(&PathBuf::from(mod_folder), side),
        _ => Err("a mod folder, its crate's or a container's, to package; or `new`, a mod folder and its side -- common, server or client".to_string()),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(why_not) => {
            eprintln!("mod_packager: {why_not}");
            ExitCode::FAILURE
        }
    }
}

/// The mod in `mod_folder` packaged in `run`, its sourced dependencies
/// first: its zip's path, or why not.
fn package_the_mod(mod_folder: &std::path::Path, run: &mut sources::PackagingRun) -> Result<PathBuf, String> {
    let mod_folder = &mod_folder.canonicalize().map_err(|io_error| format!("{}: {io_error}", mod_folder.display()))?;
    let premanifest = premanifest::read_premanifest(mod_folder)?;
    run.in_progress.push(mod_folder.clone());
    sources::package_the_sources(mod_folder, &premanifest, run)?;
    run.in_progress.pop();
    let dependencies = dependencies::read_dependencies(mod_folder, &premanifest)?;
    // A container's crate is none: nothing built, nothing to check.
    let mut library = None;
    if !premanifest.document().is_container {
        building::package_the_game()?;
        let flags = building::packaging_flags(mod_folder, &premanifest, &dependencies)?;
        let built = building::build_the_mod(mod_folder, &premanifest, &flags)?;
        set_up_check::check_set_ups(mod_folder, &premanifest, &built, &flags)?;
        library = Some(built);
    }
    let package = packaging::make_the_package(mod_folder, &premanifest, library.as_deref(), &dependencies)?;
    println!("{}: packaged, {}", premanifest.document().name, package.display());
    run.packaged.push(sources::PackagedMod { folder: mod_folder.clone(), package: package.clone() });
    Ok(package)
}
