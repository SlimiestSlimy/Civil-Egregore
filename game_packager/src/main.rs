//! The game's packager: the game built in release, then laid out as the
//! folder it ships as -- `target/civil_egregore/` in the workspace's
//! target folder, what runs beside what it needs.
//!
//! `cargo package_game` (an alias in `.cargo/config.toml`).
//!
//! The folder holds the game's binaries, its libraries beside them, and
//! libstd, which they all share: the game alone, no mod -- the base game
//! is packaged into it by `cargo distribute_game`. Today the libraries
//! alone; the server's and the client's binaries, the client's library
//! and Bevy's join it as they come.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The game's folder, inside the target folder.
const GAME_FOLDER: &str = "civil_egregore";
/// The game's libraries, each a crate of the game's built as a shared
/// library. The mod packager keeps the same list, by side.
const GAME_LIBRARIES: [&str; 3] = ["utilities", "protocol", "server"];
/// The game's binaries, each built by a crate of the game's. None yet.
const GAME_BINARIES: [&str; 0] = [];

/// The game packaged, and where it was laid out said; or why not.
fn main() -> ExitCode {
    match package_the_game() {
        Ok(game_folder) => {
            println!("the game laid out in {}", game_folder.display());
            ExitCode::SUCCESS
        }
        Err(why_not) => {
            eprintln!("game_packager: {why_not}");
            ExitCode::FAILURE
        }
    }
}

/// The game built in release and laid out in its folder, made anew
/// each time: the folder's path.
fn package_the_game() -> Result<PathBuf, String> {
    let repository_root = repository_root();
    build_the_game(&repository_root)?;
    let target_folder = target_folder(&repository_root);
    let built_folder = target_folder.join("release");
    let game_folder = target_folder.join(GAME_FOLDER);
    if game_folder.exists() {
        fs::remove_dir_all(&game_folder).map_err(|error| format!("{}: {error}", game_folder.display()))?;
    }
    fs::create_dir_all(&game_folder).map_err(|error| format!("{}: {error}", game_folder.display()))?;
    for binary in GAME_BINARIES {
        copy_into(&built_folder.join(format!("{binary}{}", std::env::consts::EXE_SUFFIX)), &game_folder)?;
    }
    for library in GAME_LIBRARIES {
        copy_into(&built_folder.join(shared_library_file(library)), &game_folder)?;
    }
    copy_into(&standard_library()?, &game_folder)?;
    Ok(game_folder)
}

/// The repository's root: the folder above this packager's own.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the packager is a folder of the repository's root").to_path_buf()
}

/// Where cargo builds into: `CARGO_TARGET_DIR` if it is set, else the
/// repository's `target/`.
fn target_folder(repository_root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or(repository_root.join("target"), PathBuf::from)
}

/// The game built in release, by the cargo that runs this packager:
/// the crate of each library, its binaries with it.
fn build_the_game(repository_root: &Path) -> Result<(), String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.args(["build", "--release"]).current_dir(repository_root);
    for library in GAME_LIBRARIES {
        command.args(["-p", library]);
    }
    let built = command.status().map_err(|error| format!("cargo could not be run: {error}"))?;
    if built.success() { Ok(()) } else { Err(format!("the game's build failed ({built})")) }
}

/// The file name the platform gives the shared library `library`
/// (`libserver.so`, `server.dll`).
fn shared_library_file(library: &str) -> String {
    format!("{}{library}{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX)
}

/// libstd as a shared library, the one every binary and library of the
/// game links: in the target library folder of the rustc that built them.
fn standard_library() -> Result<PathBuf, String> {
    let printed = Command::new("rustc").args(["--print", "target-libdir"]).output().map_err(|error| format!("rustc could not be run: {error}"))?;
    let library_folder = PathBuf::from(String::from_utf8_lossy(&printed.stdout).trim());
    let standard_library_start = format!("{}std-", std::env::consts::DLL_PREFIX);
    let folder_entries = fs::read_dir(&library_folder).map_err(|error| format!("{}: {error}", library_folder.display()))?;
    folder_entries.flatten().map(|entry| entry.path()).find(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(&standard_library_start) && name.ends_with(std::env::consts::DLL_SUFFIX))).ok_or(format!("no shared libstd in {}", library_folder.display()))
}

/// `file` copied into `folder`, under its own name.
fn copy_into(file: &Path, folder: &Path) -> Result<(), String> {
    let file_name = file.file_name().ok_or(format!("{} names no file", file.display()))?;
    fs::copy(file, folder.join(file_name)).map(|_| ()).map_err(|error| format!("{}: {error}", file.display()))
}
