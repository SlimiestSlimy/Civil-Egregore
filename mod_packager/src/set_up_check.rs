//! A mod's set up functions checked, its library never opened
//! ([`check_set_ups`]): a Rust file made of its premanifest, compiled
//! and linked against the mod's built library, never run. For each
//! function, `const _: TypeSetUp = <mod>::<function>;` -- there, at the
//! crate's root, of a set up function's signature (E0425, E0308
//! otherwise) -- and its name declared in an `unsafe extern "Rust"`
//! block, kept by a `#[used]` static: the linker must find it exported
//! by that very name, unmangled ("undefined symbol" otherwise). Built as
//! a program, so every name is resolved at link, on every OS, with no
//! flag. What it cannot see: mod_packager/known_bugs.md.

use crate::building::on_the_games_toolchain;
use crate::premanifest::Premanifest;
use std::path::Path;

/// Where, in a mod's crate, the check is made: its file, and the
/// program never run.
const CHECK_FOLDER: &str = "target/civil_egregore_set_up_check";

/// The set up functions the premanifest `premanifest` of the mod in
/// `mod_folder` names, checked against its library `library`, built with
/// `flags`: or why not.
pub fn check_set_ups(mod_folder: &Path, premanifest: &Premanifest, library: &Path, flags: &[String]) -> Result<(), String> {
    let mut set_ups: Vec<&str> = premanifest.document().types.iter().filter_map(|each_type| each_type.set_up()).collect();
    set_ups.sort_unstable();
    set_ups.dedup();
    if set_ups.is_empty() {
        return Ok(());
    }
    let mod_name = &premanifest.document().name;
    let mut source = String::from("// Made by the mod packager, each time it runs: a check, never run.\nuse protocol::type_registry::{Export, TypeRegistryHandle, TypeSetUp};\n\n");
    for set_up in &set_ups {
        source += &format!("const _: TypeSetUp = {mod_name}::{set_up};\n");
    }
    source += "\nunsafe extern \"Rust\" {\n";
    for set_up in &set_ups {
        source += &format!("    safe fn {set_up}(registry: &TypeRegistryHandle<'_>) -> Result<Export, String>;\n");
    }
    source += &format!("}}\n\n#[used]\nstatic SET_UPS: [TypeSetUp; {}] = [{}];\n\nfn main() {{}}\n", set_ups.len(), set_ups.join(", "));
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
            "{mod_name}'s set up functions, as rustc said above: each one its premanifest names must be `pub`, at its crate's root, `#[unsafe(no_mangle)]`, and a `fn(&TypeRegistryHandle<'_>) -> Result<Export, String>` (checked by {})",
            file.display()
        ));
    }
    Ok(())
}
