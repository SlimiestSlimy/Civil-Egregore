//! The host: a world run on a thread of its own for a client -- a
//! window -- that asks it for the cells of its viewport, never the
//! other way round; each call is sent to the thread and done between
//! ticks (`docs/server.md`, "The host").

mod host_thread;
use host_thread::HostThread;

mod frame;

use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use protocol::server_parameters::CleanServerParameters;
use protocol::type_registry::FrozenTypeRegistry;
use protocol::world_types::RegisterTypes;
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

/// What the host sends its client, in the order it happens.
#[derive(Clone)]
pub enum HostSent {
    /// A world runs, the `n`-th the host has run, of these types: sent
    /// once, as it starts to run, before any frame of it -- what the
    /// client draws its frames by.
    WorldRuns(u64, Arc<FrozenTypeRegistry>),
    /// A frame asked for: boxed, the types' message small beside it.
    Frame(Box<ViewportFrame>),
}

/// What a client asks of the host, sent to its thread by a [`Host`]'s
/// calls.
enum HostRequest {
    /// Some of the viewport's superchunks: answered with a [`Frame`] --
    /// unless no world runs, when nothing is.
    AskFrame(FrameAsk),
    /// Stop ticking, or go on.
    SetPaused(bool),
    /// Tick so many times a second, or flat out.
    SetPace(Option<u32>),
    /// Make a world in the folder these parameters name, as they say --
    /// created, set up of the host's game's types, and run -- in place of
    /// any run.
    MakeWorld(CleanServerParameters),
    /// Run the world of this name in the worlds' folder in place of any
    /// run: hot in its halos, as it was saved. Refused, the world run
    /// goes on (the next frame's [`Frame::said`]).
    OpenWorld(String),
    /// Run this world, started already, in place of any run, of no name:
    /// what a headless server runs from its start.
    RunWorld(Box<crate::World>),
    /// A client's call of the function whose ID is this, with these words
    /// for arguments: answered, or refused, on the sender.
    Call(u64, Vec<u64>, Sender<Result<Vec<u64>, String>>),
    /// Save the world run under this name in the worlds' folder, its
    /// name from then ([`Frame::named`]).
    Save(String),
}

/// The host, as a client holds it: each call sent to the host's thread,
/// done there between two ticks, in the order called. Each says whether
/// the host was still there to be told. Cloned, each clone tells the
/// one host.
#[derive(Clone)]
pub struct Host {
    /// Where the calls go.
    request_sender: Sender<HostRequest>,
}

impl Host {
    /// Starts the host on a thread of its own, no world run yet, worlds
    /// opened of `register_types`: the host, and where what it sends comes
    /// back ([`HostSent`]). It stops once the host is dropped.
    pub fn start_on_thread(register_types: RegisterTypes) -> (Self, Receiver<HostSent>) {
        let (request_sender, request_receiver) = channel();
        let (sender, host_sent) = channel();
        thread::Builder::new().name("host".to_string()).spawn(move || HostThread::new(register_types).serve_requests(&request_receiver, &sender)).expect("a thread for the host");
        (Self { request_sender }, host_sent)
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

    /// Makes a world in the folder `parameters` name ([`WORLD`]), as they
    /// say, of the host's game's types, and runs it in place of any run
    /// ([`crate::make_world`]).
    pub fn make_world(&self, parameters: CleanServerParameters) -> bool {
        self.request_sender.send(HostRequest::MakeWorld(parameters)).is_ok()
    }

    /// Runs the world named `world_name` in the worlds' folder in place of
    /// any run: hot in its halos, as it was saved. Refused, the world
    /// run goes on (the next frame's [`Frame::said`]).
    pub fn open_world(&self, world_name: String) -> bool {
        self.request_sender.send(HostRequest::OpenWorld(world_name)).is_ok()
    }

    /// The answer to a client's call of the function whose ID is `call`,
    /// with `arguments`, answered by the world running between two ticks;
    /// or why it is refused: no world runs, no function a client may call
    /// is of the ID, or it would not take them. Waits for it.
    pub fn call(&self, call: u64, arguments: Vec<u64>) -> Result<Vec<u64>, String> {
        let (answer_sender, answer) = channel();
        self.request_sender.send(HostRequest::Call(call, arguments, answer_sender)).map_err(|_| "the host is gone".to_string())?;
        answer.recv().map_err(|_| "the host is gone".to_string())?
    }

    /// Runs `world`, started already ([`crate::start_world`]), in place of
    /// any run, of no name.
    pub fn run_world(&self, world: crate::World) -> bool {
        self.request_sender.send(HostRequest::RunWorld(Box::new(world))).is_ok()
    }

    /// Saves the world run under `world_name` in the worlds' folder, its name
    /// from then ([`Frame::named`]).
    pub fn save_world(&self, world_name: String) -> bool {
        self.request_sender.send(HostRequest::Save(world_name)).is_ok()
    }
}
