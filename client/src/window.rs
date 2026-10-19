//! The window: a Bevy window, its server started and connected to as it
//! opens ([`crate::server_end`]). It shows nothing yet.

use crate::server_end::{ServerEnd, Shown};
use bevy::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

/// The window's server: its process, ended with the window, and what it
/// shows -- each world's types and frames -- read by nothing yet.
#[derive(Resource)]
struct WindowServer {
    /// The server's process, and where it is told and asked.
    server_end: ServerEnd,
    /// What the server shows, in the order it sends it.
    shown: Mutex<Receiver<Shown>>,
}

/// Opens the window, its server started, and runs until it is closed.
pub fn run_window() {
    let (server_end, shown) = ServerEnd::start();
    let server_calls = crate::client_api::ServerCalls::through(server_end.link.clone());
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "Civil Egregore".to_string(), ..default() }), ..default() }))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(server_calls)
        .insert_resource(WindowServer { server_end, shown: Mutex::new(shown) })
        .add_systems(Startup, spawn_camera)
        .run();
}

/// The window's camera: what shows the clear colour.
fn spawn_camera(mut bevy_commands: Commands) {
    bevy_commands.spawn(Camera2d);
}
