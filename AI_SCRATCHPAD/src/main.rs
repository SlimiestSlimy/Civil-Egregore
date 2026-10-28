//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. No part of the program: nothing
//! depends on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <package>...`: the mods read,
//! registered and loaded as the server loads them, every type listed,
//! and each sca proc with what it is sampled on.

use protocol::type_registry::TypeRegistry;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let packages: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let manifests = protocol::manifests::read_mods(&packages).map_err(|refusal| refusal.to_string())?;
    let server_mods = server::mod_loading::load_server_mods(&TypeRegistry::of_manifests(&manifests));
    for registered in server_mods.type_registry.types() {
        println!("{:#018x} {}", registered.type_id(), registered.qualified_name());
    }
    for sca_proc in &server_mods.sca_procs {
        println!("sca proc {:#018x}: sampled on {:x?}, {:?} a tick", sca_proc.type_id, sca_proc.sampled_plane_type_ids, sca_proc.chance_per_tick);
    }
    Ok(())
}
