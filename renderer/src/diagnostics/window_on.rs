//! The window opened on a world a diagnostic or a test ticks, running:
//! the same world, to be watched (`docs/renderer.md`, "Watching a
//! test's world").

use crate::link::Link;
use bevy::prelude::*;
use gui::Screen;
use server::Start;

/// The world the window is opened on: none once the host was told.
#[derive(Resource)]
struct Watched(Option<Start>);

/// Tells the host to make the world, once, and shows it in place of
/// the main menu.
fn show(mut watched: ResMut<Watched>, mut link: ResMut<Link>, mut screen: ResMut<Screen>) {
    if let Some(start) = watched.0.take() {
        _ = link.host.make_world(start);
        link.forget_asked();
        (link.first_view, *screen) = (true, Screen::World);
    }
}

/// Opens the window on the world started as `start` says, running,
/// until it is closed.
pub fn open(start: Start) {
    let mut window = crate::window();
    window.insert_resource(Watched(Some(start))).add_systems(Update, show.after(crate::hud::hud));
    window.run();
}
