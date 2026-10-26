//! A mod's crate made, empty but ready to package ([`make_a_new_mod`]),
//! as `cargo new` makes one: its Cargo.toml -- a dylib, a workspace of
//! its own, the game's source to develop with -- the game's toolchain,
//! its premanifest, an empty `dependencies/`, and a lib.rs saying what a
//! set up function is. Its name its folder's, its premanifest read back
//! as every premanifest is, before anything is written.

use crate::building::game_libraries;
use crate::premanifest::PREMANIFEST_FILE;
use cdnf::writer::CdnfWriter;
use protocol::manifest_format::{read_mod_document, FormatError};
use protocol::mod_packages::DEPENDENCIES_FOLDER;
use std::path::Path;

/// A new mod's lib.rs.
const LIBRARY_SOURCE: &str = "//! A mod of Civil Egregore's.
//!
//! Each type its premanifest names with a `set up` has its set up
//! function here, at the crate's root, by that name:
//!
//! ```ignore
//! #[unsafe(no_mangle)]
//! pub fn example_set_up(registry: &TypeRegistryHandle<'_>) -> Result<TypeCode, String> {
//!     ...
//! }
//! ```
//!
//! (`protocol::type_registry::{TypeCode, TypeRegistryHandle}`). What it
//! returns is what the game, and the mods depending on this one, get of
//! the type. Packaged by `cargo package_mod <this folder>`, from the
//! game's folder.
";

/// The mod's crate made in `mod_folder`, none there yet, of the side
/// `side`: or why not.
pub fn make_a_new_mod(mod_folder: &Path, side: &str) -> Result<(), String> {
    if mod_folder.exists() {
        return Err(format!("{}: there already", mod_folder.display()));
    }
    let name = mod_folder.file_name().and_then(|file_name| file_name.to_str()).unwrap_or_default();
    if !protocol::type_names::is_a_name(name) {
        return Err(format!("{}: its folder's name, the mod's, one character or more, each of A-Z, a-z, 0-9 and _", mod_folder.display()));
    }
    let premanifest = CdnfWriter::new().section(name).list().pair("side").text(side).pair("version").text("0.1").end_list().end_section().finish();
    let document = cdnf::parse(&premanifest).map_err(|cdnf_error| cdnf_error.what)?;
    let document = read_mod_document(&document).map_err(|FormatError { section, what }| format!("{section}: {what}"))?;
    let game_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().map_err(|io_error| format!("the game's folder: {io_error}"))?;
    // Developed with the source of each game library packaging gives it.
    let game_sources: String = game_libraries(document.side).iter().map(|library| format!("{library} = {{ path = '{}' }}\n", game_folder.join(library).display())).collect();
    let channel = option_env!("GAME_CHANNEL").ok_or("the game's toolchain: unknown, the packager not built by rustup")?;
    let cargo_toml = format!(
        "[package]\nname = \"{name}\"\nedition = \"2024\"\n\n# A mod: one library, loaded by the game.\n[lib]\ncrate-type = [\"dylib\"]\n\n# A workspace of its own, wherever it is.\n[workspace]\n\n# The source it is developed with; packaging drops it, the game's\n# compiled libraries given instead.\n[target.'cfg(not(civil_egregore_packaging))'.dependencies]\n{game_sources}"
    );
    let toolchain = format!("# The game's toolchain: the mod's source built with it, as the packager\n# builds the mod.\n[toolchain]\nchannel = \"{channel}\"\n");
    let files = [("Cargo.toml", cargo_toml.as_str()), ("rust-toolchain.toml", &toolchain), (PREMANIFEST_FILE, &premanifest), (".gitignore", "target/\n"), ("src/lib.rs", LIBRARY_SOURCE)];
    for folder in [mod_folder.join("src"), mod_folder.join(DEPENDENCIES_FOLDER)] {
        std::fs::create_dir_all(&folder).map_err(|io_error| format!("{}: {io_error}", folder.display()))?;
    }
    for (file, text) in files {
        std::fs::write(mod_folder.join(file), text).map_err(|io_error| format!("{}: {io_error}", mod_folder.join(file).display()))?;
    }
    Ok(())
}
