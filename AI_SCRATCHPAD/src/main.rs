//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. No part of the program: nothing
//! depends on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- hash <file>`: a file's hash,
//! as a manifest gives a library's.
//! `cargo run --release -p AI_SCRATCHPAD -- load <side> <mod folder>...`:
//! the mods read, registered and loaded for the side, every type listed.

use protocol::kinds::Side;
use protocol::type_names::TypeName;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["hash", file] => {
            let bytes = std::fs::read(file).map_err(|io_error| format!("{file}: {io_error}"))?;
            println!("0x{:x}", <sha2::Sha256 as sha2::Digest>::digest(&bytes));
        }
        ["load", side, mod_folders @ ..] => {
            let side = match *side {
                "server" => Side::Server,
                "client" => Side::Client,
                other => return Err(format!("a side, server or client: not {other}")),
            };
            let mod_folders: Vec<PathBuf> = mod_folders.iter().map(PathBuf::from).collect();
            let manifests = protocol::manifests::read_mods(&mod_folders).map_err(|refusal| refusal.to_string())?;
            let loaded = protocol::type_registry::TypeRegistry::of_manifests(&manifests).load(side)?;
            println!("{} libraries loaded", loaded.library_count());
            for registered in loaded.registry().types() {
                let code = loaded.code_of(registered.type_id()).map_or("no code".to_string(), |code| match code.downcast_ref::<TypeName>() {
                    Some(type_name) => format!("code, downcast to a TypeName: {type_name}"),
                    None => "code of a type the probe does not know".to_string(),
                });
                println!("{:016x} {} -- {code}", registered.type_id(), registered.full_name());
            }
        }
        _ => return Err("hash <file>, or load <side> <mod folder>...".to_string()),
    }
    Ok(())
}
