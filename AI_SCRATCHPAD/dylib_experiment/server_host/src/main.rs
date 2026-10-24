//! The server binary: the world's mods' common and server bundles
//! registered, the registry frozen (here: no longer mutable), each server
//! bundle started, a few ticks run.

use engine_server::engine_common::{self as common, Registry, Side};
use engine_server::{ServerHandle, ServerStart};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let world = PathBuf::from(std::env::args().nth(1).ok_or("give the world's folder")?);
    common::WORLD_NAME.set("experiment world".to_string()).unwrap();
    let declared = common::declared_bundles(&world.join("mods"))?;
    let ordered = common::in_loading_order(&declared, &[Side::Common, Side::Server])?;
    println!("server loads: {:?}", ordered.iter().map(|bundle| &bundle.name).collect::<Vec<_>>());
    let mut registry = Registry::default();
    let opened = common::register_bundles(&ordered, &mut registry)?;
    let registry = registry;
    let mut handle = ServerHandle::default();
    for (bundle, library) in opened.iter().filter(|(bundle, _)| bundle.side == Side::Server) {
        // SAFETY: a server bundle's start is a ServerStart, the same toolchain's.
        let start: ServerStart = unsafe { std::mem::transmute(library.symbol(common::START_SYMBOL)?) };
        start(&registry, &mut handle).map_err(|why| format!("{}: {why}", bundle.name))?;
    }
    for _ in 0..3 {
        for line in handle.tick() {
            println!("  {line}");
        }
    }
    println!("server maps the client engine: {}, bevy: {}", common::mapped("libengine_client"), common::mapped("bevy"));
    Ok(())
}
