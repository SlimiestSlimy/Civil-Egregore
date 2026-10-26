//! A mod's crate built against the game and its dependencies as
//! compiled ([`build_the_mod`]): never against their source. Cargo run
//! on the game's toolchain with the packager's flags alone -- the
//! game's, `--cfg civil_egregore_packaging` to drop what the mod's
//! Cargo.toml lists for developing, `--extern` for each library the mod
//! may use, `-L` for what those were built on -- in
//! `CARGO_ENCODED_RUSTFLAGS`, which every config's rustflags give way
//! to: the game's, the mod's, its developer's.

use crate::dependencies::{Dependencies, EXTRACTED_FOLDER};
use crate::premanifest::Premanifest;
use protocol::kinds::Side;
use std::path::{Path, PathBuf};
use std::process::Command;
use utilities::dynamic_library::library_file_name;

/// Where, in a mod's crate, cargo builds it to package it: a folder of
/// cargo's own, made by it, apart from the developer's builds.
const BUILD_FOLDER: &str = "target/civil_egregore_build";

/// The cfg the packager's builds have: what a mod's Cargo.toml lists
/// under `cfg(not(civil_egregore_packaging))` -- the source it is
/// developed with -- dropped.
const PACKAGING_CFG: &str = "civil_egregore_packaging";

/// The game's libraries a mod may use.
const GAME_LIBRARIES: [&str; 2] = ["utilities", "protocol"];

/// The flags the crate of the mod in `mod_folder` -- whole, its path
/// canonical -- whose premanifest is `premanifest` and dependencies
/// `dependencies`, is built with: the game's, the packaging cfg, the
/// libraries it may use and where what they were built on is. Or why
/// none.
pub fn packaging_flags(mod_folder: &Path, premanifest: &Premanifest, dependencies: &Dependencies) -> Result<Vec<String>, String> {
    let game_libraries = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/release/deps").canonicalize().map_err(|io_error| format!("the game's libraries: {io_error}"))?;
    let extracted = mod_folder.join(EXTRACTED_FOLDER);
    let mut flags: Vec<String> = env!("GAME_RUSTFLAGS").split('\u{1f}').filter(|flag| !flag.is_empty()).map(str::to_string).collect();
    flags.extend(["--cfg".to_string(), PACKAGING_CFG.to_string()]);
    for name in GAME_LIBRARIES {
        flags.extend(["--extern".to_string(), format!("{name}={}", game_libraries.join(library_file_name(name)).display())]);
    }
    // A mod uses what its side may depend on: indirect dependencies too, loaded before it all the same.
    for manifest in dependencies.manifests.iter().filter(|manifest| !manifest.document().container) {
        if manifest.document().side == Side::Common || manifest.document().side == premanifest.document().side {
            flags.extend(["--extern".to_string(), format!("{}={}", manifest.document().name, extracted.join(library_file_name(&manifest.document().name)).display())]);
        }
    }
    for folder in [&game_libraries, &extracted] {
        flags.extend(["-L".to_string(), format!("dependency={}", folder.display())]);
    }
    Ok(flags)
}

/// The crate of the mod in `mod_folder` -- whole, its path canonical --
/// whose premanifest is `premanifest`, built in release with `flags`:
/// its library's path, or why not.
pub fn build_the_mod(mod_folder: &Path, premanifest: &Premanifest, flags: &[String]) -> Result<PathBuf, String> {
    let target_folder = mod_folder.join(BUILD_FOLDER);
    // Cargo never looks at an --extern library: the mod built anew, whatever changed.
    if target_folder.exists() {
        cargo(mod_folder, &target_folder, flags, &["clean", "--release", "-p", &premanifest.document().name])?;
    }
    cargo(mod_folder, &target_folder, flags, &["build", "--release"])?;
    let library = target_folder.join("release").join(library_file_name(&premanifest.document().name));
    if !library.is_file() {
        return Err(format!("{}: not built -- the crate in {} must be named {} and build a dylib (`[lib] crate-type = [\"dylib\"]` in its Cargo.toml)", library.display(), mod_folder.display(), premanifest.document().name));
    }
    Ok(library)
}

/// Cargo run in `mod_folder` with `arguments`, its target folder
/// `target_folder` and the rustflags `flags` alone: or why it failed.
fn cargo(mod_folder: &Path, target_folder: &Path, flags: &[String], arguments: &[&str]) -> Result<(), String> {
    let mut command = on_the_games_toolchain("cargo");
    command.current_dir(mod_folder).args(arguments).arg("--target-dir").arg(target_folder).env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"));
    let status = command.status().map_err(|io_error| format!("cargo: {io_error}"))?;
    if !status.success() {
        return Err(format!("cargo {}: failed in {}", arguments.join(" "), mod_folder.display()));
    }
    Ok(())
}

/// The program `program` -- cargo, rustc -- to run on the game's
/// toolchain.
pub fn on_the_games_toolchain(program: &str) -> Command {
    let mut command = Command::new(program);
    if let Some(toolchain) = option_env!("GAME_TOOLCHAIN") {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }
    command
}
