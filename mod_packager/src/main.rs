//! The mod packager: a mod -- its crate, or a container's folder --
//! made into the zip it ships as. Its premanifest read and checked; its
//! dependencies' zips, in its `dependencies/` folder, read and checked;
//! the mod built against the game and them; its set up functions
//! checked; its manifest written; all of it zipped, and read back as the
//! game reads it.
//!
//! `cargo package_mod <mod folder>`, from the game (an alias in its
//! `.cargo/config.toml`).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::path::PathBuf;
use std::process::ExitCode;

mod building;
mod dependencies;
mod packaging;
mod premanifest;
mod set_up_check;

/// The mod in the folder given packaged; or why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [mod_folder] = arguments.as_slice() else {
        eprintln!("mod_packager: one mod folder, its crate's or a container's");
        return ExitCode::FAILURE;
    };
    match package_the_mod(&PathBuf::from(mod_folder)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why_not) => {
            eprintln!("mod_packager: {why_not}");
            ExitCode::FAILURE
        }
    }
}

/// The mod in `mod_folder` packaged: or why not.
fn package_the_mod(mod_folder: &std::path::Path) -> Result<(), String> {
    let mod_folder = &mod_folder.canonicalize().map_err(|io_error| format!("{}: {io_error}", mod_folder.display()))?;
    let premanifest = premanifest::read_premanifest(mod_folder)?;
    let dependencies = dependencies::read_dependencies(mod_folder, &premanifest)?;
    // A container's crate is none: nothing built, nothing to check.
    let mut library = None;
    if !premanifest.document().container {
        let flags = building::packaging_flags(mod_folder, &premanifest, &dependencies)?;
        let built = building::build_the_mod(mod_folder, &premanifest, &flags)?;
        set_up_check::check_set_ups(mod_folder, &premanifest, &built, &flags)?;
        library = Some(built);
    }
    let package = packaging::make_the_package(mod_folder, &premanifest, library.as_deref(), &dependencies)?;
    println!("{}: packaged, {}", premanifest.document().name, package.display());
    Ok(())
}
