//! The client binary: the world's mods' common and client bundles
//! registered, each client bundle started on a Bevy App, a few frames run.

use engine_client::bevy::app::TaskPoolPlugin;
use engine_client::bevy::prelude::*;
use engine_client::engine_common::{self as common, Registry, Side};
use engine_client::{ClientStart, FrameNumber, Said};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let world = PathBuf::from(std::env::args().nth(1).ok_or("give the world's folder")?);
    common::WORLD_NAME.set("experiment world".to_string()).unwrap();
    let declared = common::declared_bundles(&world.join("mods"))?;
    let ordered = common::in_loading_order(&declared, &[Side::Common, Side::Client])?;
    println!("client loads: {:?}", ordered.iter().map(|bundle| &bundle.name).collect::<Vec<_>>());
    let mut registry = Registry::default();
    let opened = common::register_bundles(&ordered, &mut registry)?;
    let registry = registry;
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default()).init_resource::<Said>().insert_resource(FrameNumber(0)).add_systems(First, |mut frame: ResMut<FrameNumber>| frame.0 += 1);
    for (bundle, library) in opened.iter().filter(|(bundle, _)| bundle.side == Side::Client) {
        // SAFETY: a client bundle's start is a ClientStart, the same toolchain's.
        let start: ClientStart = unsafe { std::mem::transmute(library.symbol(common::START_SYMBOL)?) };
        start(&registry, &mut app).map_err(|why| format!("{}: {why}", bundle.name))?;
    }
    for _ in 0..3 {
        app.update();
    }
    for line in &app.world().resource::<Said>().0 {
        println!("  {line}");
    }
    Ok(())
}
