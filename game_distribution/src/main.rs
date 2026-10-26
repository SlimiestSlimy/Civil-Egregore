//! The game as it ships: packaged by the game packager in
//! `target/civil_egregore/`, then the base game packaged by the mod
//! packager -- its mods from their sources, as its premanifest gives
//! them -- and its zip copied into the game's folder, beside the game's
//! libraries and binaries.
//!
//! `cargo distribute_game` (an alias in `.cargo/config.toml`).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The game distributed, and where said; or why not.
fn main() -> ExitCode {
    match distribute_the_game() {
        Ok(game_folder) => {
            println!("the game, the base game in it, in {}", game_folder.display());
            ExitCode::SUCCESS
        }
        Err(why_not) => {
            eprintln!("game_distribution: {why_not}");
            ExitCode::FAILURE
        }
    }
}

/// The game packaged, the base game packaged and copied into its
/// folder: the folder's path.
fn distribute_the_game() -> Result<PathBuf, String> {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the distribution is a folder of the repository's root").to_path_buf();
    run_cargo(&repository_root, &["run", "--release", "-p", "game_packager"])?;
    run_cargo(&repository_root, &["run", "--release", "-p", "mod_packager", "--", "basegame"])?;
    let game_folder = repository_root.join("target").join("civil_egregore");
    let basegame = repository_root.join("basegame").join("target").join("basegame.zip");
    let copied = game_folder.join("basegame.zip");
    fs::copy(&basegame, &copied).map_err(|error| format!("{} to {}: {error}", basegame.display(), copied.display()))?;
    Ok(game_folder)
}

/// The cargo that runs this distribution run in `repository_root` with
/// `arguments`: or why it failed.
fn run_cargo(repository_root: &Path, arguments: &[&str]) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo).args(arguments).current_dir(repository_root).status().map_err(|error| format!("cargo could not be run: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed ({status})", arguments.join(" "))) }
}
