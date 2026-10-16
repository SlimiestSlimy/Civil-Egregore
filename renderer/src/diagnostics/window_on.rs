//! The window opened on a world an in-bin test or a test ticks, running:
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
fn make_watched_world(mut watched: ResMut<Watched>, mut link: ResMut<Link>, mut screen: ResMut<Screen>) {
    if let Some(start) = watched.0.take() {
        _ = link.host.make_world(start);
        link.forget_asked();
        (link.first_view, *screen) = (true, Screen::World);
    }
}

/// Shows the world, whichever ticker's it is, in place of the main
/// menu: the window of [`show_worlds`]. And closes the window once
/// what ticked has stopped and nothing else ticks
/// (`server::host::shown::no_ticker_is_left`).
fn show_world(mut screen: ResMut<Screen>, mut exit: MessageWriter<AppExit>) {
    if *screen != Screen::World {
        *screen = Screen::World;
    }
    if server::host::shown::no_ticker_is_left() {
        exit.write(AppExit::Success);
    }
}

/// Opens the window on the worlds that come to be shown
/// (`server::host::shown`), one after another, until it is closed
/// or the last ticker has stopped:
/// each a world a test or an in-bin test ticks, drawn as the window
/// draws any. Says so and opens none if such a window is open
/// already.
pub fn show_worlds() -> Result<(), String> {
    let link = Link::of_shown_worlds().ok_or_else(|| format!("a window showing worlds is open already: port {} is taken", server::host::shown::PORT))?;
    let mut window = crate::window_on(link);
    window.add_systems(Update, show_world.after(crate::hud::hud));
    window.run();
    Ok(())
}

/// Opens the window on the world started as `start` says, running,
/// until it is closed.
pub fn open_window(start: Start) {
    let mut window = crate::window();
    window.insert_resource(Watched(Some(start))).add_systems(Update, make_watched_world.after(crate::hud::hud));
    window.run();
}
