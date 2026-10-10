//! The host: a world run on a thread of its own for a client -- a
//! window -- that asks it for the cells of its viewport, never the
//! other way round; each call is sent to the thread and done between
//! ticks (`docs/server.md`, "The host").

mod host_thread;
use host_thread::HostThread;

pub mod frame;
pub mod terrain;

use crate::Start;
use utilities::tuning::Tuning;
use worldgen::Generation;
use frame::{Ask, Frame};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// Ticks a second a world is held to unless told otherwise: the
/// game's target.
pub const TARGET_PACE: u32 = 256;

/// Ticks from one line of the census to the next.
pub const CENSUS_EVERY: u64 = 1000;

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
enum Request {
    /// Some of the viewport's superchunks: answered with a [`Frame`] --
    /// unless no world runs, when nothing is.
    Sync(Ask),
    /// Stop ticking, or go on.
    Pause(bool),
    /// Tick so many times a second, or flat out.
    Pace(Option<u32>),
    /// Make a world as this says, and run it in place of any run.
    New(Start),
    /// Run the world of this name in the worlds' folder in place of any
    /// run: hot in its halos, as it was saved. Refused, the world run
    /// goes on (the next frame's [`Frame::said`]).
    Open(String),
    /// Save the world run under this name in the worlds' folder, its
    /// name from then ([`Frame::named`]).
    Save(String),
    /// Make the world run again from its start, generated as this says:
    /// of many read between two ticks, the last alone.
    Reset(Box<Generation>),
}

/// Where the census of a run is kept: the flock and the grass every
/// [`CENSUS_EVERY`] ticks, written as the run goes, so a run closed at
/// any time leaves what it came to.
pub fn census_path() -> PathBuf {
    crate::transient_data::TRANSIENT_DATA.measurements().join("census.csv")
}

/// Starts the census afresh for a world of `seed`: its file, with what
/// was run and the columns' names. `None`, and no census kept, if it
/// cannot be made.
fn census(seed: u64) -> Option<BufWriter<File>> {
    let path = census_path();
    create_dir_all(path.parent()?).ok()?;
    let mut file = BufWriter::new(File::create(path).ok()?);
    writeln!(file, "# seed {}", utilities::seed::hex(seed)).ok()?;
    writeln!(file, "tick,seconds,pace,sheep,grass").ok()?;
    Some(file)
}

/// The host, as a client holds it: each call sent to the host's thread,
/// done there between two ticks, in the order called. Each says whether
/// the host was still there to be told.
pub struct Host {
    /// Where the calls go.
    requests: Sender<Request>,
    /// Where the terrain is asked of.
    terrain: terrain::TerrainAsker,
}

impl Host {
    /// Starts the host on a thread of its own, no world run yet: the
    /// host, and where its frames come back. It stops once the host is
    /// dropped.
    pub fn start() -> (Self, Receiver<Frame>) {
        let (requests, asked) = channel();
        let (answers, frames) = channel();
        let terrain = terrain::start();
        let told = terrain.clone();
        thread::Builder::new().name("host".to_string()).spawn(move || HostThread::telling(told).run(&asked, &answers)).expect("a thread for the host");
        (Self { requests, terrain: terrain::TerrainAsker { requests: terrain } }, frames)
    }

    /// Where the terrain of the world run is asked of, what no frame
    /// brings: a handle of its own, for a thread that may wait.
    pub fn terrain(&self) -> terrain::TerrainAsker {
        self.terrain.clone()
    }

    /// Asks for some of the viewport's superchunks: answered with a
    /// [`Frame`] -- unless no world runs, when nothing is.
    pub fn sync(&self, ask: Ask) -> bool {
        self.requests.send(Request::Sync(ask)).is_ok()
    }

    /// Stops ticking, or goes on.
    pub fn pause(&self, paused: bool) -> bool {
        self.requests.send(Request::Pause(paused)).is_ok()
    }

    /// Ticks so many times a second, or flat out.
    pub fn pace(&self, pace: Option<u32>) -> bool {
        self.requests.send(Request::Pace(pace)).is_ok()
    }

    /// Makes a world as `start` says, and runs it in place of any run.
    pub fn make_world(&self, start: Start) -> bool {
        self.requests.send(Request::New(start)).is_ok()
    }

    /// Runs the world named `name` in the worlds' folder in place of
    /// any run: hot in its halos, as it was saved. Refused, the world
    /// run goes on (the next frame's [`Frame::said`]).
    pub fn open_world(&self, name: String) -> bool {
        self.requests.send(Request::Open(name)).is_ok()
    }

    /// Saves the world run under `name` in the worlds' folder, its name
    /// from then ([`Frame::named`]).
    pub fn save_world(&self, name: String) -> bool {
        self.requests.send(Request::Save(name)).is_ok()
    }

    /// Makes the world run again from its start, its generation as
    /// `tuning` has it now (`Generation::from_tuning`), the rest as it
    /// started: its seed, size, sheep, hot entity and camera. A slider
    /// dragged calls it each frame; of calls with no frame asked between, only the last
    /// is done.
    pub fn reset(&self, tuning: &Tuning) -> bool {
        self.requests.send(Request::Reset(Box::new(Generation::from_tuning(tuning)))).is_ok()
    }
}
