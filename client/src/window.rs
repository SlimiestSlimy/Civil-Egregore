//! The window: a Bevy window, its server started and connected to as it
//! opens ([`crate::server_end`]), a world made as its program says, its windows ([`windows`]: the world's gui
//! types), and the world drawn ([`drawing`]) as its camera sees it
//! ([`camera`]: WASD or the arrows, the mouse, its wheel, Q and E). Space pauses; `=` and `-` double and halve the pace,
//! `0` holds it to the simulation's own, `F` lets it run flat out.

mod camera;
mod drawing;
mod windows;

use crate::server_end::{ServerEnd, Shown};
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::theme::UiTheme;
use bevy::feathers::FeathersPlugins;
use bevy::prelude::*;
use drawing::Drawing;
use protocol::server_parameters::CleanServerParameters;
use protocol::world_types::RegisterTypes;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

/// The window's title, before what it says of the world.
const TITLE: &str = "Civil Egregore";

/// The window's server: its process, ended with the window, and what it
/// shows -- each world's types and frames.
#[derive(Resource)]
struct WindowServer {
    /// The server's process, and where it is told and asked.
    server_end: ServerEnd,
    /// What the server shows, in the order it sends it.
    shown: Mutex<Receiver<Shown>>,
}

/// Opens the window, its server started, and runs until it is closed:
/// each world's types registered by `register_types`, the game's, and
/// the world it opens with made as `first_world` says -- until the
/// window has its menus, the program's to say.
pub fn run_window(register_types: RegisterTypes, first_world: CleanServerParameters) {
    let (server_end, shown) = ServerEnd::start();
    let server_calls = crate::client_api::ServerCalls::through(server_end.link.clone());
    App::new()
        .add_plugins((DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: TITLE.to_string(), ..default() }), ..default() }), FeathersPlugins))
        .insert_resource(UiTheme(create_dark_theme()))
        .init_resource::<windows::Windows>()
        .add_observer(windows::take_world_types)
        .add_observer(windows::open_window)
        .add_systems(Last, windows::let_go_of_focus)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(server_calls)
        .insert_resource(WindowServer { server_end, shown: Mutex::new(shown) })
        .insert_resource(Drawing::new(register_types))
        .insert_resource(FirstWorld(first_world))
        .add_systems(Startup, (camera::spawn_camera, make_a_world))
        .add_systems(Update, (camera::move_camera, drawing::take_shown, drawing::ask_frames, drawing::show_painted, keys_to_the_server, say_in_the_title).chain())
        .run();
}

/// The parameters of the world the window opens with.
#[derive(Resource)]
struct FirstWorld(CleanServerParameters);

/// The world the window opens with made: until the window has its
/// menus, the one its program says.
fn make_a_world(window_server: Res<WindowServer>, first_world: Res<FirstWorld>) {
    window_server.server_end.link.make_world(first_world.0.clone());
}

/// The keys the server is told by: Space pauses or goes on; `=` and `-`
/// double and halve the pace, `0` holds it to the simulation's own,
/// `F` lets it run flat out.
fn keys_to_the_server(keys: Res<ButtonInput<KeyCode>>, window_server: Res<WindowServer>, drawing: Res<Drawing>, mut paused: Local<bool>) {
    let link = &window_server.server_end.link;
    if keys.just_pressed(KeyCode::Space) {
        *paused = !*paused;
        link.set_paused(*paused);
    }
    let Some(frame) = &drawing.last_frame else {
        return;
    };
    let pace_now = frame.held_pace.unwrap_or(frame.target_pace);
    if keys.just_pressed(KeyCode::Equal) {
        link.set_pace(Some(pace_now.saturating_mul(2)));
    }
    if keys.just_pressed(KeyCode::Minus) {
        link.set_pace(Some((pace_now / 2).max(1)));
    }
    if keys.just_pressed(KeyCode::Digit0) {
        link.set_pace(Some(frame.target_pace));
    }
    if keys.just_pressed(KeyCode::KeyF) {
        link.set_pace(None);
    }
}

/// The window's title saying how the world runs and how fast the window
/// draws, every half second.
fn say_in_the_title(drawing: Res<Drawing>, time: Res<Time>, mut window: Single<&mut Window>, mut since_said: Local<(f32, u32)>) {
    let (seconds, frames) = &mut *since_said;
    (*seconds, *frames) = (*seconds + time.delta_secs(), *frames + 1);
    if *seconds < 0.5 {
        return;
    }
    let frames_a_second = *frames as f32 / *seconds;
    (*seconds, *frames) = (0.0, 0);
    window.title = match &drawing.last_frame {
        Some(frame) => {
            let pace = frame.held_pace.map_or("flat out".to_string(), |held_pace| format!("held to {held_pace}"));
            format!("{TITLE} -- tick {}, {:.0} ticks a second ({pace}), {} entities, {} hot superchunks in view, frames {:.0}% of the server's time -- {frames_a_second:.0} fps", frame.tick, frame.ticks_a_second, frame.entity_count, frame.hot.len(), 100.0 * frame.sync_share)
        }
        None => format!("{TITLE} -- no world yet -- {frames_a_second:.0} fps"),
    };
}
