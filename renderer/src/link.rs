//! The window's side of the host (`server::host`): where it is asked,
//! where its frames come back painted ([`crate::paint`]), and what the
//! window tells it -- a world made, opened or saved as the menus say,
//! paused, paced.

use crate::paint::Picture;
use bevy::prelude::*;
use coordinates::WORLD_MIDDLE;
use gui::options::Options;
use gui::{Captured, Make, Open, Save};
use server::host::frame::{Ask, Frame};
use server::host::{Request, TARGET_PACE};
use server::{Size, Start};
use std::hash::{BuildHasher, RandomState};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use utilities::tuning::{self, SHEEP};
use worldgen::{has_land_about, Generation};

/// Seeds tried after one drawn at random for one with land about the
/// origin, where the sheep start, before the one drawn is taken.
const LAND_TRIES: u64 = 256;

/// The host, as the window holds it: where to ask, where the answers
/// come, and what it was last told.
#[derive(Resource)]
pub struct Link {
    /// Where requests go.
    pub requests: Sender<Request>,
    /// Where frames come back, painted.
    pub pictures: Mutex<Receiver<Picture>>,
    /// Whether a frame was asked for and has not come yet.
    pub waiting: bool,
    /// Seconds since a frame was last asked for.
    pub since: f32,
    /// What was last asked for: the next frame goes on from it, round
    /// the superchunks in view.
    pub asked: Option<Ask>,
    /// Whether the world is paused.
    pub paused: bool,
    /// Ticks a second it is held to, or flat out.
    pub pace: Option<u32>,
}

impl Link {
    /// The host, at the game's pace, asked nothing yet; its frames
    /// painted on a thread of their own.
    pub fn start() -> Self {
        let (requests, frames) = server::host::start();
        Self { requests, pictures: Mutex::new(crate::paint::start(frames)), waiting: false, since: f32::INFINITY, asked: None, paused: false, pace: Some(TARGET_PACE) }
    }

    /// Sends `request` for a world in place of the one run: a frame
    /// asked of the one before is not waited for.
    fn run(&mut self, request: Request) {
        _ = self.requests.send(request);
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
    /// Superchunks in view.
    pub in_view: u32,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    pub detail: u32,
    /// Pixels along a cell's side, seen from near; 0 if not.
    pub near_pixels: u32,
    /// Cells along a pixel's side of the map; 0 if it is not shown.
    pub map: u32,
}

/// What a world made from the menus starts from: the seed given, or
/// one drawn at random -- the first from it with land where the sheep
/// start, if one is near -- and the rest as the sliders have it.
fn start(seed: Option<u64>) -> Start {
    let now = tuning::now();
    let generation = Generation::from_tuning(&now);
    let seed = seed.unwrap_or_else(|| {
        let drawn = RandomState::new().hash_one(std::time::SystemTime::now());
        (drawn..drawn.saturating_add(LAND_TRIES)).find(|&seed| has_land_about(seed, &generation.shape, WORLD_MIDDLE)).unwrap_or(drawn)
    });
    Start { seed, generation, size: Size::from_tuning(&now), threads: None, sheep: now[SHEEP].max(0.0) as usize }
}

/// Tells the host of each world the menus make, open or save; and the
/// options the name of the world run.
pub fn menus(mut link: ResMut<Link>, mut make: MessageReader<Make>, mut open: MessageReader<Open>, mut save: MessageReader<Save>, mut options: ResMut<Options>, seen: Res<Seen>) {
    for made in make.read() {
        link.run(Request::New(start(made.0)));
    }
    for opened in open.read() {
        link.run(Request::Open(opened.0.clone()));
    }
    for saved in save.read() {
        _ = link.requests.send(Request::Save(saved.0.clone()));
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
        _ = link.requests.send(Request::Pause(link.paused));
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
    _ = link.requests.send(Request::Pace(pace));
}
