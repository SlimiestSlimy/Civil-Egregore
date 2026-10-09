//! Civil Egregore on the screen: a Bevy window showing a world run by
//! the host (`server::host`) on a thread of its own -- dirt, grass,
//! trees, water and sheep, on ground lit by its height: slopes shaded,
//! cliffs casting shadows, and from near, steps and walls drawn at
//! their edges.
//!
//! It opens on the main menu (`gui`): a world is made there, its seed
//! typed or drawn and the rest as the sliders have it -- its size,
//! whether it is forced hot, its sheep, how it is generated -- or one
//! saved is opened. Only then is the host asked for anything.
//!
//! The window is the one that asks: each time it has shown a frame, it
//! asks the host for the hot superchunks of its viewport -- whatever it
//! should render ([`frames`]) -- and the host answers with their cells
//! as its last tick left them, which a third thread turns into pixels
//! ([`paint`]). The three share nothing else, so none waits on another.
//! In map mode, at any zoom, a fourth draws the map from generation
//! alone ([`map`]) and no cells are asked for.
//!
//! | module | what it is |
//! |---|---|
//! | [`link`] | the host as the window holds it: what the menus and the keys tell it |
//! | [`frames`] | frames asked for and shown |
//! | [`view`] | the plane, the camera, and steering it |
//! | [`overlays`] | boundaries, labels and heights over the world |
//! | [`hud`] | the text over the world |
//! | [`map`] | the world from far off |
//! | [`paint`], [`near`], [`ground`] | cells into pixels, on the painter's thread |
//!
//! | key | what it does |
//! |---|---|
//! | arrows, WASD, or dragging with the left button | move the view |
//! | the wheel, or `Q` and `E` | zoom |
//! | space | pause, and go on |
//! | `T` | tick flat out, or at the game's pace |
//! | `F11` | the window over the whole screen, or not |
//! | `[` and `]` | halve and double the pace |
//! | `B` | show the superchunks' boundaries, or not, and near enough each one's Morton index and `(x, y)` |
//! | `C` | the same of the chunks |
//! | `H` | show every cell's height, from near enough to read them |
//! | `M` | map mode, or not: the map in place of the cells, at any zoom |
//! | `P` | draw the mesh's lines over the map |
//! | `U` | the sliders' menu, or none: the near view's shading over a world, everything while one is made |
//! | Escape | the options, or none: going on, saving the world, opening one of the worlds' folder, and leaving Civil Egregore |

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]
// On Windows a window alone: no console opened beside it, as one is
// for a program that does not say so. It prints nothing.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod frames;
mod ground;
mod hud;
mod link;
mod map;
mod near;
mod overlays;
mod paint;
mod view;

use bevy::prelude::*;
use gui::{Gui, Screen};

/// Opens the window on the main menu, and runs until it is closed.
pub fn run() {
    // Only over a world: a world shown, steered and asked for.
    let world = (view::fullscreen, link::keys, view::steer, overlays::toggle, overlays::boundaries, overlays::labels, overlays::heights, frames::show, frames::ask, map::far).chain().run_if(resource_equals(Screen::World));
    App::new()
        .add_plugins(
            DefaultPlugins
                // A cell a pixel, sharp however near.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin { primary_window: Some(Window { title: "Civil Egregore".to_string(), ..default() }), ..default() }),
        )
        .add_plugins(Gui { worlds: || server::worlds_in(&utilities::settings::worlds()) })
        .insert_resource(link::Link::start())
        .insert_resource(map::MapLink::start())
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<link::Seen>()
        .init_resource::<view::Sprites>()
        .init_resource::<overlays::Shown>()
        .add_systems(Startup, (view::spawn, frames::spawn, map::spawn, overlays::spawn, hud::spawn))
        .add_systems(Update, (link::menus, link::shading, world, hud::hud).chain().after(gui::Worked))
        .run();
}
