//! The window's side of the host (`server::host`): where it is asked,
//! where its frames come back painted ([`crate::paint`]), and what the
//! window tells it -- a world made, opened or saved as the menus say,
//! paused, paced -- and the painter the shading the sliders set.

use crate::paint::Picture;
use bevy::prelude::*;
use gui::options::Options;
use gui::{Captured, CurrentTuning, Make, Open, Save, Screen};
use server::host::frame::{Ask, Frame};
use server::host::{Host, TARGET_PACE};
use server::Start;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use utilities::tuning::Tuning;
use server::host::terrain_seen::Generation;

/// The host, as the window holds it: where to ask, where the answers
/// come, and what it was last told.
#[derive(Resource)]
pub struct Link {
    /// The host, asked by calls.
    pub host: Host,
    /// Where frames come back, painted.
    pub pictures: Mutex<Receiver<Picture>>,
    /// Where the sliders' numbers go to the painter, for its shading.
    pub shading: Sender<Tuning>,
    /// Whether a frame was asked for and has not come yet.
    pub waiting: bool,
    /// Seconds since a frame was last asked for.
    pub since: f32,
    /// What was last asked for: the next frame goes on from it, round
    /// the viewport's hot superchunks.
    pub asked: Option<Ask>,
    /// Whether the world is paused.
    pub paused: bool,
    /// Ticks a second it is held to, or flat out.
    pub pace: Option<u32>,
    /// How the sliders last had worlds generated: none until they are
    /// first seen.
    pub generation: Option<Generation>,
    /// Whether the view is to be put over the next world shown: one
    /// made or opened, not one remade.
    pub first_view: bool,
}

impl Link {
    /// The host, at the game's pace, asked nothing yet; its frames
    /// painted on a thread of their own.
    pub fn start() -> Self {
        let (host, frames) = Host::start();
        let (shading, tunings) = channel();
        Self { host, pictures: Mutex::new(crate::paint::start(frames, tunings)), shading, waiting: false, since: f32::INFINITY, asked: None, paused: false, pace: Some(TARGET_PACE), generation: None, first_view: true }
    }

    /// Forgets a frame asked of the world run: another is to run in its
    /// place, and the frame is not waited for.
    fn forget_asked(&mut self) {
        (self.waiting, self.asked) = (false, None);
    }
}

/// What the last frame said of the world, and how the window drew it.
#[derive(Resource, Default)]
pub struct Seen {
    /// The last frame, its cells gone into pixels: none until a world
    /// runs.
    pub frame: Option<Frame>,
    /// Superchunks the frame painted.
    pub painted: usize,
    /// Seconds of the painter's thread the frame took.
    pub paint_seconds: f64,
    /// Superchunks in the viewport: none in map mode.
    pub viewport_superchunks: u32,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    pub detail: u32,
    /// Pixels along a cell's side, seen from near; 0 if not.
    pub near_pixels: u32,
    /// Cells along a pixel's side of the map; 0 if it is not shown.
    pub map: u32,
}

/// Tells the host of each world the menus make, open or save; and the
/// options the name of the world run.
pub fn menus(mut link: ResMut<Link>, mut make: MessageReader<Make>, mut open: MessageReader<Open>, mut save: MessageReader<Save>, mut options: ResMut<Options>, seen: Res<Seen>) {
    for made in make.read() {
        _ = link.host.make_world(Start::from_tuning(made.seed, &made.tuning));
        link.forget_asked();
        link.first_view = true;
    }
    for opened in open.read() {
        _ = link.host.open_world(opened.0.clone());
        link.forget_asked();
        link.first_view = true;
    }
    for saved in save.read() {
        _ = link.host.save_world(saved.0.clone());
    }
    if let Some(frame) = &seen.frame
        && options.bypass_change_detection().named() != frame.named.as_deref()
    {
        options.name(frame.named.clone());
    }
}

/// Pauses and paces the world by the keys.
pub fn keys(mut link: ResMut<Link>, keys: Res<ButtonInput<KeyCode>>, captured: Res<Captured>) {
    if captured.keys {
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        link.paused = !link.paused;
        _ = link.host.pause(link.paused);
    }
    let pace = if keys.just_pressed(KeyCode::KeyT) {
        if link.pace.is_some() { None } else { Some(TARGET_PACE) }
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        Some((link.pace.unwrap_or(TARGET_PACE) / 2).max(1))
    } else if keys.just_pressed(KeyCode::BracketRight) {
        Some(link.pace.unwrap_or(TARGET_PACE).saturating_mul(2))
    } else {
        return;
    };
    link.pace = pace;
    _ = link.host.pace(pace);
}

/// Sends the painter the sliders' numbers whenever they change: its
/// shading from near is theirs.
pub fn shading(link: Res<Link>, tuning: Res<CurrentTuning>) {
    if tuning.is_changed() {
        _ = link.shading.send(tuning.0);
    }
}

/// Has the host make the world run again whenever the sliders change
/// how worlds are generated: from its start, the view left where it
/// is. Compared with what the sliders last said, not with the world's
/// own: a slider of the shading remakes no world opened of other
/// numbers.
pub fn generation(mut link: ResMut<Link>, tuning: Res<CurrentTuning>, screen: Res<Screen>, seen: Res<Seen>) {
    if !tuning.is_changed() {
        return;
    }
    let generation = Generation::from_tuning(&tuning.0);
    if link.generation.replace(generation).is_some_and(|last| last != generation) && *screen == Screen::World && seen.frame.is_some() {
        _ = link.host.reset(&tuning.0);
        link.forget_asked();
    }
}
