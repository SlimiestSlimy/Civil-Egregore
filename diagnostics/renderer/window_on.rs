//! The window opened on a world an in-bin test or a test ticks, running:
//! the same world, to be watched (`docs/renderer.md`, "Watching a
//! test's world").

use crate::link::HostLink;
use bevy::prelude::*;
use gui::ShownScreen;
use server::WorldStart;

/// The world the window is opened on: none once the host was told.
#[derive(Resource)]
struct WatchedStart(Option<WorldStart>);

/// Tells the host to make the world, once, and shows it in place of
/// the main menu.
fn make_watched_world(mut watched_start: ResMut<WatchedStart>, mut host_link: ResMut<HostLink>, mut shown_screen: ResMut<ShownScreen>) {
    if let Some(world_start) = watched_start.0.take() {
        _ = host_link.host.make_world(world_start);
        host_link.forget_asked();
        (host_link.first_view, *shown_screen) = (true, ShownScreen::World);
    }
}

/// Shows the world, whichever ticker's it is, in place of the main
/// menu: the window of [`show_worlds`]. And closes the window once
/// what ticked has stopped and nothing else ticks
/// (`server::host::shown::no_ticker_is_left`).
fn show_world(mut shown_screen: ResMut<ShownScreen>, mut exit_writer: MessageWriter<AppExit>) {
    if *shown_screen != ShownScreen::World {
        *shown_screen = ShownScreen::World;
    }
    if server::host::shown::no_ticker_is_left() {
        exit_writer.write(AppExit::Success);
    }
}

/// Opens the window on the worlds that come to be shown
/// (`server::host::shown`), one after another, until it is closed
/// or the last ticker has stopped:
/// each a world a test or an in-bin test ticks, drawn as the window
/// draws any. Says so and opens none if such a window is open
/// already.
pub fn show_worlds() -> Result<(), String> {
    let host_link = HostLink::of_shown_worlds().ok_or_else(|| format!("a window showing worlds is open already: port {} is taken", server::host::shown::SHOWN_WORLDS_PORT))?;
    let mut window = crate::window_on(host_link);
    window.add_systems(Update, show_world.after(crate::hud::write_hud));
    window.run();
    Ok(())
}

/// Opens the window on the world started as `world_start` says, running,
/// until it is closed.
pub fn open_window(world_start: WorldStart) {
    let mut window = crate::window();
    window.insert_resource(WatchedStart(Some(world_start))).add_systems(Update, make_watched_world.after(crate::hud::write_hud));
    window.run();
}
