//! Civil Egregore on the screen: a Bevy window showing a world run by
//! the host (`server::host`) on a thread of its own. The window asks,
//! the host copies, a third thread paints; a fourth draws the map.
//!
//! The design, and the keys: `docs/renderer.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
mod climate_labels;
mod frames;
mod ground;
mod hud;
mod link;
mod map;
mod mipmaps;
mod near;
mod overlays;
mod paint;
pub mod transient_data;
mod view;

use bevy::prelude::*;
use gui::{GuiPlugin, ShownScreen};

/// Opens the window on the main menu, and runs until it is closed.
pub fn run_window() {
    window().run();
}

/// The window, on the main menu, its world run by a host of its own:
/// not yet run.
fn window() -> App {
    window_on(link::HostLink::start_own_host())
}

/// The window, on the main menu, its world the one `host_link` reaches:
/// not yet run.
fn window_on(host_link: link::HostLink) -> App {
    // Only over a world: a world shown, steered and asked for.
    let world = (view::toggle_fullscreen, link::pause_and_pace_by_keys, view::steer_view, overlays::toggle, overlays::lay_boundaries, overlays::label_corners, overlays::heights, climate_labels::climates, frames::show_frames, frames::ask_next_frame, map::show_map).chain().run_if(resource_equals(ShownScreen::World));
    let mut window_app = App::new();
    window_app.add_plugins(
            DefaultPlugins
                // A cell a pixel, sharp however near.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin { primary_window: Some(Window { title: "Civil Egregore".to_string(), ..default() }), ..default() }),
        )
        .add_plugins((GuiPlugin { worlds: || server::worlds_in(&utilities::settings::worlds()) }, mipmaps::Mipmaps))
        .insert_resource(map::MapLink::start_map_link(host_link.host.terrain()))
        .insert_resource(host_link)
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<link::LastFrameSeen>()
        .init_resource::<view::SuperchunkSprites>()
        .init_resource::<overlays::OverlaysShown>()
        .add_systems(Startup, (view::spawn_camera, frames::spawn_near_view, map::spawn_map_view, overlays::spawn_overlays, climate_labels::spawn_climate_labels, hud::spawn_hud))
        .add_systems(Update, (link::tell_host_of_menus, link::send_shading, link::remake_on_generation_change, world, hud::write_hud).chain().after(gui::MenusWorked));
    window_app
}
