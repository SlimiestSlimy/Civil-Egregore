//! A mod's set up statics checked, its library never opened
//! ([`check_set_ups`]): a Rust file made of its premanifest, compiled
//! and linked against the mod's built library, never run. For each
//! static, `const _: <its kind's set up> = <mod>::<static>;` -- there,
//! at the crate's root, of its kind's set up type (E0425, E0308
//! otherwise) -- and its name declared in an `unsafe extern "Rust"`
//! block, kept by a `#[used]` static of its own: the linker must find it exported
//! by that very name, unmangled ("undefined symbol" otherwise). Built as
//! a program, so every name is resolved at link, on every OS, with no
//! flag. What it cannot see: mod_packager/known_bugs.md.

use crate::building::on_the_games_toolchain;
use crate::premanifest::Premanifest;
use protocol::type_kinds::Kind;
use std::path::Path;

/// Where, in a mod's crate, the check is made: its file, and the
/// program never run.
const CHECK_FOLDER: &str = "target/civil_egregore_set_up_check";

/// The type of the set up of a type of `kind`, a kind with code, as the
/// check names it.
fn set_up_type(kind: &Kind) -> &'static str {
    match kind {
        Kind::ScaProc { .. } => "server::sca_proc::ScaProcSetUp",
        Kind::Mod { .. } | Kind::Plane { .. } => unreachable!("no set up: the premanifest's reader refuses one"),
    }
}

/// The set up statics the premanifest `premanifest` of the mod in
/// `mod_folder` names, checked against its library `library`, built with
/// `flags`: or why not.
pub fn check_set_ups(mod_folder: &Path, premanifest: &Premanifest, library: &Path, flags: &[String]) -> Result<(), String> {
    let mut set_ups: Vec<(&str, &str)> = premanifest.document().types.iter().filter_map(|each_type| Some((each_type.set_up_static_name()?, set_up_type(each_type.kind())))).collect();
    set_ups.sort_unstable();
    set_ups.dedup();
    if set_ups.is_empty() {
        return Ok(());
    }
    let mod_name = &premanifest.document().name;
    let mut source = String::from("// Made by the mod packager, each time it runs: a check, never run.\n\n");
    for (set_up, set_up_type) in &set_ups {
        source += &format!("const _: {set_up_type} = {mod_name}::{set_up};\n");
    }
    source += "\nunsafe extern \"Rust\" {\n";
    for (set_up, set_up_type) in &set_ups {
        source += &format!("    safe static {set_up}: {set_up_type};\n");
    }
    source += "}\n\n";
    for (set_up, set_up_type) in &set_ups {
        source += &format!("#[used]\nstatic KEPT_{set_up}: &{set_up_type} = &{set_up};\n");
    }
    source += "\nfn main() {}\n";
    let folder = mod_folder.join(CHECK_FOLDER);
    let file = folder.join("set_up_check.rs");
    std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&file, source)).map_err(|io_error| format!("{}: {io_error}", file.display()))?;
    let status = on_the_games_toolchain("rustc")
        .args(["--edition", "2024", "--crate-type", "bin", "--crate-name", "set_up_check", "-o"])
        .arg(folder.join("set_up_check"))
        .arg(&file)
        .args(flags)
        .arg("--extern")
        .arg(format!("{mod_name}={}", library.display()))
        .status()
        .map_err(|io_error| format!("rustc: {io_error}"))?;
    if !status.success() {
        return Err(format!(
            "{mod_name}'s set up statics, as rustc said above: each one its premanifest names must be `pub`, at its crate's root, `#[unsafe(no_mangle)]`, and of its kind's set up type -- a sca proc's `server::sca_proc::ScaProcSetUp` (checked by {})",
            file.display()
        ));
    }
    Ok(())
}
