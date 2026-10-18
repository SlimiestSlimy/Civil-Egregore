//! The host: a world run on a thread of its own for a client -- a
//! window -- that asks it for the cells of its viewport, never the
//! other way round; each call is sent to the thread and done between
//! ticks (`docs/server.md`, "The host").

mod host_thread;
use host_thread::HostThread;

mod frame;
pub mod terrain;

use crate::WorldStart;
use utilities::tuning::Tuning;
use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use protocol::type_registry::FrozenTypes;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Ticks a second a world is held to unless told otherwise: the
/// game's target, a second of the simulation in a second.
pub const TARGET_PACE: u32 = crate::simulation::simulation_time::TICKS_A_SIMULATION_SECOND as u32;

/// The share of a tick's time the host gives to copying a frame's
/// superchunks before the next tick, when it has no time to spare: a
/// frame is answered a little at a time, and costs the ticks this much
/// however much is asked.
const FRAMES_SHARE: f64 = 0.125;

/// How far behind its pace the host may fall and still catch up:
/// ticks made late by a frame or a sleep are made up, a stall is not.
const CATCH_UP: Duration = Duration::from_millis(250);

/// What a client asks of the host, sent to its thread by a [`Host`]'s
/// calls.
#[derive(Clone, Debug)]
enum HostRequest {
    /// Some of the viewport's superchunks: answered with a [`Frame`] --
    /// unless no world runs, when nothing is.
    AskFrame(FrameAsk),
    /// Stop ticking, or go on.
    SetPaused(bool),
    /// Tick so many times a second, or flat out.
    SetPace(Option<u32>),
    /// Make a world as this says, and run it in place of any run.
    MakeWorld(WorldStart),
    /// Run the world of this name in the worlds' folder in place of any
    /// run: hot in its halos, as it was saved. Refused, the world run
    /// goes on (the next frame's [`Frame::said`]).
    OpenWorld(String),
    /// Save the world run under this name in the worlds' folder, its
    /// name from then ([`Frame::named`]).
    Save(String),
    /// Make the world run again from its start, generated as these
    /// sliders say: of many read between two ticks, the last alone.
    Reset(Box<Tuning>),
}

/// The host, as a client holds it: each call sent to the host's thread,
/// done there between two ticks, in the order called. Each says whether
/// the host was still there to be told.
pub struct Host {
    /// Where the calls go.
    request_sender: Sender<HostRequest>,
    /// Where the terrain is asked of.
    terrain: terrain::TerrainAsker,
}

impl Host {
    /// Starts the host on a thread of its own, no world run yet, worlds
    /// made of `frozen_types` and opened among them: the host, and where
    /// its frames come back. It stops once the host is dropped.
    pub fn start_on_thread(frozen_types: Arc<FrozenTypes>) -> (Self, Receiver<ViewportFrame>) {
        let (request_sender, request_receiver) = channel();
        let (frame_sender, frame_receiver) = channel();
        let terrain = terrain::start_terrain_thread();
        let terrain_for_host = terrain.clone();
        thread::Builder::new().name("host".to_string()).spawn(move || HostThread::telling_terrain(terrain_for_host, frozen_types).serve_requests(&request_receiver, &frame_sender)).expect("a thread for the host");
        (Self { request_sender, terrain: terrain::TerrainAsker { request_sender: terrain } }, frame_receiver)
    }

    /// Where the terrain of the world run is asked of, what no frame
    /// brings: a handle of its own, for a thread that may wait.
    pub fn terrain(&self) -> terrain::TerrainAsker {
        self.terrain.clone()
    }

    /// Asks for some of the viewport's superchunks: answered with a
    /// [`Frame`] -- unless no world runs, when nothing is.
    pub fn ask_frame(&self, frame_ask: FrameAsk) -> bool {
        self.request_sender.send(HostRequest::AskFrame(frame_ask)).is_ok()
    }

    /// Stops ticking, or goes on.
    pub fn set_paused(&self, ticking_paused: bool) -> bool {
        self.request_sender.send(HostRequest::SetPaused(ticking_paused)).is_ok()
    }

    /// Ticks so many times a second, or flat out.
    pub fn set_pace(&self, pace_held_to: Option<u32>) -> bool {
        self.request_sender.send(HostRequest::SetPace(pace_held_to)).is_ok()
    }

    /// Makes a world as `world_start` says, and runs it in place of any run.
    pub fn make_world(&self, world_start: WorldStart) -> bool {
        self.request_sender.send(HostRequest::MakeWorld(world_start)).is_ok()
    }

    /// Runs the world named `world_name` in the worlds' folder in place of
    /// any run: hot in its halos, as it was saved. Refused, the world
    /// run goes on (the next frame's [`Frame::said`]).
    pub fn open_world(&self, world_name: String) -> bool {
        self.request_sender.send(HostRequest::OpenWorld(world_name)).is_ok()
    }

    /// Saves the world run under `world_name` in the worlds' folder, its name
    /// from then ([`Frame::named`]).
    pub fn save_world(&self, world_name: String) -> bool {
        self.request_sender.send(HostRequest::Save(world_name)).is_ok()
    }

    /// Makes the world run again from its start, its generation as
    /// `tuning` has it now, as its generation reads the sliders
    /// (`WorldgenCode::numbers_of_tuning`), the rest as it
    /// started: its seed, size, sheep, hot entity and camera. A slider
    /// dragged calls it each frame; of calls with no frame asked between, only the last
    /// is done.
    pub fn reset(&self, tuning: &Tuning) -> bool {
        self.request_sender.send(HostRequest::Reset(Box::new(*tuning))).is_ok()
    }
}
