//! The window's side of the server ([`crate::server_end`]): where it is asked,
//! where its frames come back painted ([`crate::renderer::paint`]), and what the
//! window tells it -- a world made, opened or saved as the menus say,
//! paused, paced -- and the painter the shading the sliders set.

use crate::renderer::paint::PaintedFrame;
use bevy::prelude::*;
use crate::gui::options::Options;
use crate::gui::{CapturedInput, CurrentTuning, MakeWorld, OpenWorld, Save, ShownScreen};
use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use crate::server_end::{ServerEnd, ServerLink};
use protocol::pipe::messages::TARGET_PACE;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use utilities::tuning::{tuned_number, SliderGroup, Tuning};

/// The server, as the window holds it: where to ask, where the answers
/// come, and what it was last told.
#[derive(Resource)]
pub struct HostLink {
    /// The server's process, and where it is asked.
    pub server_end: ServerEnd,
    /// Where frames come back, painted.
    pub painted_pictures: Mutex<Receiver<PaintedFrame>>,
    /// Where the sliders' numbers go to the painter, for its shading.
    pub shading_sender: Sender<Tuning>,
    /// Whether a frame was asked for and has not come yet.
    pub awaiting_frame: bool,
    /// Seconds since a frame was last asked for.
    pub seconds_since_asked: f32,
    /// What was last asked for: the next frame goes on from it, round
    /// the viewport's hot superchunks.
    pub last_ask: Option<FrameAsk>,
    /// Whether the world is paused.
    pub world_paused: bool,
    /// Ticks a second it is held to, or flat out.
    pub held_pace: Option<u32>,
    /// The sliders of how worlds are generated, as they last were: none
    /// until they are first seen.
    pub last_generation: Option<Vec<f32>>,
    /// Whether the view is to be put over the next world shown: one
    /// made or opened, not one remade.
    pub first_view: bool,
}

impl HostLink {
    /// A server of the window's own, at the game's pace, asked nothing
    /// yet; its frames painted on a thread of their own.
    pub fn start_own_server() -> Self {
        let (server_end, frame_receiver) = ServerEnd::start();
        let (shading_sender, tunings) = channel();
        let painted_pictures = Mutex::new(crate::renderer::paint::start_painter(frame_receiver, tunings, server_end.link.clone()));
        Self { server_end, painted_pictures, shading_sender, awaiting_frame: false, seconds_since_asked: f32::INFINITY, last_ask: None, world_paused: false, held_pace: Some(TARGET_PACE), last_generation: None, first_view: true }
    }

    /// Where the server is told and asked.
    pub fn server(&self) -> &ServerLink {
        &self.server_end.link
    }

    /// Forgets a frame asked of the world run: another is to run in its
    /// place, and the frame is not waited for.
    pub fn forget_asked(&mut self) {
        (self.awaiting_frame, self.last_ask) = (false, None);
    }
}

/// What the last frame said of the world, and how the window drew it.
#[derive(Resource, Default)]
pub struct LastFrameSeen {
    /// The last frame, its cells gone into pixels: none until a world
    /// runs.
    pub last_frame: Option<ViewportFrame>,
    /// Superchunks the frame painted.
    pub painted_superchunks: usize,
    /// Seconds of the painter's thread the frame took.
    pub paint_seconds: f64,
    /// Superchunks in the viewport: none in map mode.
    pub viewport_superchunks: u32,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    pub detail_shift: u32,
    /// Pixels along a cell's side, seen from near; 0 if not.
    pub near_pixels: u32,
    /// Cells along a pixel's side of the map; 0 if it is not shown.
    pub map: u32,
    /// The climates of the superchunks the frames brought, as each
    /// last was, by where it is in the world.
    pub climates: std::collections::HashMap<(u32, u32), Vec<u64>>,
}

/// Tells the host of each world the menus make, open or save; and the
/// options the name of the world run.
pub fn tell_host_of_menus(mut host_link: ResMut<HostLink>, mut make_world_reader: MessageReader<MakeWorld>, mut open_world_reader: MessageReader<OpenWorld>, mut save: MessageReader<Save>, mut options: ResMut<Options>, last_seen: Res<LastFrameSeen>) {
    for made_world in make_world_reader.read() {
        _ = host_link.server().make_world(made_world.seed, made_world.tuning);
        host_link.forget_asked();
        host_link.first_view = true;
    }
    for opened_world in open_world_reader.read() {
        _ = host_link.server().open_world(opened_world.0.clone());
        host_link.forget_asked();
        host_link.first_view = true;
    }
    for saved_world in save.read() {
        _ = host_link.server().save_world(saved_world.0.clone());
    }
    if let Some(last_frame) = &last_seen.last_frame
        && options.bypass_change_detection().current_world_name() != last_frame.world_name.as_deref()
    {
        options.set_world_name(last_frame.world_name.clone());
    }
}

/// Pauses and paces the world by the keys.
pub fn pause_and_pace_by_keys(mut host_link: ResMut<HostLink>, pressed_keys: Res<ButtonInput<KeyCode>>, captured_input: Res<CapturedInput>) {
    if captured_input.keyboard_keys {
        return;
    }
    if pressed_keys.just_pressed(KeyCode::Space) {
        host_link.world_paused = !host_link.world_paused;
        _ = host_link.server().set_paused(host_link.world_paused);
    }
    let new_pace = if pressed_keys.just_pressed(KeyCode::KeyT) {
        if host_link.held_pace.is_some() { None } else { Some(TARGET_PACE) }
    } else if pressed_keys.just_pressed(KeyCode::BracketLeft) {
        Some((host_link.held_pace.unwrap_or(TARGET_PACE) / 2).max(1))
    } else if pressed_keys.just_pressed(KeyCode::BracketRight) {
        Some(host_link.held_pace.unwrap_or(TARGET_PACE).saturating_mul(2))
    } else {
        return;
    };
    host_link.held_pace = new_pace;
    _ = host_link.server().set_pace(new_pace);
}

/// Sends the painter the sliders' numbers whenever they change: its
/// shading from near is theirs.
pub fn send_shading(host_link: Res<HostLink>, tuning: Res<CurrentTuning>) {
    if tuning.is_changed() {
        _ = host_link.shading_sender.send(tuning.0);
    }
}

/// The numbers of every slider but the shading's: how worlds are
/// generated, and the world's own, shown only while one is set up.
fn generation_sliders(tuning: &Tuning) -> Vec<f32> {
    tuning.iter().enumerate().filter(|&(number_index, _)| tuned_number(number_index).slider_group != SliderGroup::NearViewShading).map(|(_, &number)| number).collect()
}

/// Has the host make the world run again whenever the sliders change
/// how worlds are generated: from its start, the view left where it
/// is. Compared with what the sliders last said, not with the world's
/// own: a slider of the shading remakes no world opened of other
/// numbers.
pub fn remake_on_generation_change(mut host_link: ResMut<HostLink>, tuning: Res<CurrentTuning>, shown_screen: Res<ShownScreen>, last_seen: Res<LastFrameSeen>) {
    if !tuning.is_changed() {
        return;
    }
    let new_generation = generation_sliders(&tuning.0);
    if host_link.last_generation.replace(new_generation.clone()).is_some_and(|previous_generation| previous_generation != new_generation) && *shown_screen == ShownScreen::World && last_seen.last_frame.is_some() {
        _ = host_link.server().reset(tuning.0);
        host_link.forget_asked();
    }
}
