//! A still of the window itself: a world made, the view put at so
//! many cells a pixel, and what the graphics card drew kept as a PNG
//! -- the one way to see what the card makes of the pictures, its
//! mipmaps among it, with no one looking at the window
//! (`docs/renderer.md`, "Mipmaps made on the graphics card").

use crate::link::LastFrameSeen;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use gui::{CurrentTuning, MakeWorld, ShownScreen};
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
struct WantedStill {
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
fn take_still(mut bevy_commands: Commands, mut wanted_still: ResMut<WantedStill>, mut make_world_writer: MessageWriter<MakeWorld>, mut shown_screen: ResMut<ShownScreen>, tuning: Res<CurrentTuning>, last_seen: Res<LastFrameSeen>, camera_projection: Single<&mut Projection, With<Camera2d>>, mut exit_writer: MessageWriter<AppExit>) {
    let Some(shown_for) = wanted_still.shown_for else {
        make_world_writer.write(MakeWorld { seed: Some(wanted_still.seed), tuning: tuning.0 });
        (*shown_screen, wanted_still.shown_for) = (ShownScreen::World, Some(0));
        return;
    };
    if last_seen.last_frame.is_none() {
        return;
    }
    if let Projection::Orthographic(orthographic_view) = &mut *camera_projection.into_inner() {
        orthographic_view.scale = wanted_still.cells_a_pixel;
    }
    if shown_for == SETTLED_AFTER {
        bevy_commands.spawn(Screenshot::primary_window()).observe(save_to_disk(wanted_still.path.clone()));
    }
    if shown_for == SETTLED_AFTER + WRITTEN_AFTER {
        exit_writer.write(AppExit::Success);
    }
    wanted_still.shown_for = Some(shown_for + 1);
}

/// Opens the window on the world of `seed`, `cells_a_pixel` cells a
/// screen pixel over its middle, keeps what the graphics card drew at
/// `path`, and closes it.
pub fn save_still(seed: u64, cells_a_pixel: f32, path: PathBuf) {
    let mut window = crate::window();
    window.insert_resource(WantedStill { seed, cells_a_pixel, path, shown_for: None }).add_systems(Update, take_still.after(crate::hud::write_hud));
    window.run();
}
