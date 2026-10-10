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
use gui::{Gui, Screen};

/// Opens the window on the main menu, and runs until it is closed.
pub fn run() {
    window().run();
}

/// The window, on the main menu, not yet run.
fn window() -> App {
    // Only over a world: a world shown, steered and asked for.
    let world = (view::fullscreen, link::keys, view::steer, overlays::toggle, overlays::boundaries, overlays::labels, overlays::heights, frames::show, frames::ask, map::far).chain().run_if(resource_equals(Screen::World));
    let link = link::Link::start();
    let mut app = App::new();
    app.add_plugins(
            DefaultPlugins
                // A cell a pixel, sharp however near.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin { primary_window: Some(Window { title: "Civil Egregore".to_string(), ..default() }), ..default() }),
        )
        .add_plugins((Gui { worlds: || server::worlds_in(&utilities::settings::worlds()) }, mipmaps::Mipmaps))
        .insert_resource(map::MapLink::start(link.host.terrain()))
        .insert_resource(link)
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<link::Seen>()
        .init_resource::<view::Sprites>()
        .init_resource::<overlays::Shown>()
        .add_systems(Startup, (view::spawn, frames::spawn, map::spawn, overlays::spawn, hud::spawn))
        .add_systems(Update, (link::menus, link::shading, link::generation, world, hud::hud).chain().after(gui::Worked));
    app
}
