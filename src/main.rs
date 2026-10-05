//! TileSim, from the command line: the one program beside the
//! renderer. It holds nothing but which crates have commands; the rest
//! is `utilities::commands`.
//!
//! `cargo run --release -- help`: every command of every crate, what
//! each takes and what that is if not given.
//! `cargo run --release -- server run <folder> [ticks]`
//! `cargo run --release -- tessera <tool> [...]`

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::process::ExitCode;
use utilities::commands::{program, Crate};

/// The crates with commands.
const CRATES: [Crate; 2] = [
    Crate { name: "server", does: "a world made, run and looked at, and the server's diagnostics tools", commands: &server::commands::COMMANDS },
    Crate { name: "tessera", does: "Tessera's diagnostics tools", commands: &tessera::diagnostics::tool::COMMANDS },
];

/// Hands the command line to the crate its first word names, or says why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match program("Civil_Egregore", &CRATES, &arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}



#[cfg(feature = "renderer")]
fn main() {
    tuning::start();

    let superchunks = 0;
    let pace = Some(256);

    lab::run();
    sliders::in_lab();

    let (requests, frames) = start(superchunks);
    _ = requests.send(Request::Pace(pace));
    App::new()
        .add_plugins(
            DefaultPlugins
                // A cell a pixel, sharp however near.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin { primary_window: Some(Window { title: "TileSim".to_string(), ..default() }), ..default() }),
        )
        .insert_resource(Link { requests, frames: Mutex::new(paint::start(frames)), waiting: false, since: SYNC_EVERY, asked: None, paused: false, pace })
        .insert_resource(Sprites::about_origin(superchunks))
        .insert_resource({
            let (requests, maps) = map::start();
            MapLink { requests, maps: Mutex::new(maps), asked: None, borders: false }
        })
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<Seen>()
        .init_resource::<Boundaries>()
        .add_plugins(Gui { worlds: || server::worlds_in(&utilities::settings::worlds()) })
        .add_systems(Startup, setup)
        .add_systems(Update, (fullscreen, open, recentre, steer, keys, boundaries, labels, heights, show, ask, far, hud).chain().after(gui::Worked))
        .run();
}
