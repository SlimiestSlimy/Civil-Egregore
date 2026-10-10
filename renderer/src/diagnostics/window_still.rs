//! A still of the window itself: a world made, the view put at so
//! many cells a pixel, and what the graphics card drew kept as a PNG
//! -- the one way to see what the card makes of the pictures, its
//! mipmaps among it, with no one looking at the window
//! (`docs/renderer.md`, "Mipmaps made on the graphics card").

use crate::link::Seen;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use gui::{CurrentTuning, Make, Screen};
use std::path::PathBuf;

/// Frames the world is shown for before the still is taken: every
/// superchunk of the viewport painted, and its mipmaps made.
const SETTLED_AFTER: u32 = 300;
/// Frames from the still asked for to the window closed: the still
/// written by then.
const WRITTEN_AFTER: u32 = 90;

/// What the window is opened for: a still of the world of `seed`, the
/// screen showing `cells_a_pixel` cells a pixel, kept at `path`.
#[derive(Resource)]
struct Wanted {
    /// The world's seed.
    seed: u64,
    /// Cells a screen pixel.
    cells_a_pixel: f32,
    /// The file the still is kept as.
    path: PathBuf,
    /// Frames since the world was first shown: none until it is made.
    shown_for: Option<u32>,
}

/// Makes the world, holds the view where the still is wanted, takes
/// the still once the picture has settled, and closes the window.
#[allow(clippy::too_many_arguments)]
fn take(mut commands: Commands, mut wanted: ResMut<Wanted>, mut make: MessageWriter<Make>, mut screen: ResMut<Screen>, tuning: Res<CurrentTuning>, seen: Res<Seen>, camera: Single<&mut Projection, With<Camera2d>>, mut exit: MessageWriter<AppExit>) {
    let Some(shown_for) = wanted.shown_for else {
        make.write(Make { seed: Some(wanted.seed), tuning: tuning.0 });
        (*screen, wanted.shown_for) = (Screen::World, Some(0));
        return;
    };
    if seen.frame.is_none() {
        return;
    }
    if let Projection::Orthographic(view) = &mut *camera.into_inner() {
        view.scale = wanted.cells_a_pixel;
    }
    if shown_for == SETTLED_AFTER {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(wanted.path.clone()));
    }
    if shown_for == SETTLED_AFTER + WRITTEN_AFTER {
        exit.write(AppExit::Success);
    }
    wanted.shown_for = Some(shown_for + 1);
}

/// Opens the window on the world of `seed`, `cells_a_pixel` cells a
/// screen pixel over its middle, keeps what the graphics card drew at
/// `path`, and closes it.
pub fn keep(seed: u64, cells_a_pixel: f32, path: PathBuf) {
    let mut window = crate::window();
    window.insert_resource(Wanted { seed, cells_a_pixel, path, shown_for: None }).add_systems(Update, take.after(crate::hud::hud));
    window.run();
}
