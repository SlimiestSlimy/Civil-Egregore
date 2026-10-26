//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. No part of the program: nothing
//! depends on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- hash <file>`: a file's hash,
//! as a manifest gives a library's.
//! `cargo run --release -p AI_SCRATCHPAD -- zip <archive> <file>...`:
//! the files zipped, each by the path given; `unzip <archive>` lists one.
//! `cargo run --release -p AI_SCRATCHPAD -- load <side> <package>...`:
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
        ["zip", archive_path, files @ ..] => {
            let read: Vec<(String, Vec<u8>)> = files.iter().map(|file| Ok((file.to_string(), std::fs::read(file).map_err(|io_error| format!("{file}: {io_error}"))?))).collect::<Result<_, String>>()?;
            let entries: Vec<(&str, &[u8])> = read.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
            std::fs::write(archive_path, utilities::archive::write_archive(&entries)).map_err(|io_error| format!("{archive_path}: {io_error}"))?;
        }
        ["unzip", archive_path] => {
            let archive = std::fs::read(archive_path).map_err(|io_error| format!("{archive_path}: {io_error}"))?;
            for (path, bytes) in utilities::archive::read_archive(&archive)? {
                println!("{path}: {} bytes", bytes.len());
            }
        }
        ["load", side, packages @ ..] => {
            let side = match *side {
                "server" => Side::Server,
                "client" => Side::Client,
                other => return Err(format!("a side, server or client: not {other}")),
            };
            let packages: Vec<PathBuf> = packages.iter().map(PathBuf::from).collect();
            let manifests = protocol::manifests::read_mods(&packages).map_err(|refusal| refusal.to_string())?;
            let loaded = protocol::type_registry::TypeRegistry::of_manifests(&manifests).load(side)?;
            println!("{} libraries loaded", loaded.library_count());
            for registered in loaded.registry().types() {
                let code = loaded.code_of(registered.type_id()).map_or("no code".to_string(), |code| {
                    if let Some(type_name) = code.downcast_ref::<TypeName>() {
                        format!("code, downcast to a TypeName: {type_name}")
                    } else if let Some(sca_proc) = code.downcast_ref::<Box<dyn server::sca_proc::ScaProc>>() {
                        sca_proc.call(&mut server::server_handle::ServerHandle {}, 0, utilities::coordinates::CellIndex(0));
                        "code, downcast to a sca proc, called".to_string()
                    } else {
                        "code of a type the probe does not know".to_string()
                    }
                });
                println!("{:016x} {} -- {code}", registered.type_id(), registered.full_name());
            }
        }
        _ => return Err("hash <file>, zip <archive> <file>..., unzip <archive>, or load <side> <package>...".to_string()),
    }
    Ok(())
}
