//! The host: a world run on a thread of its own, for a client -- a
//! window -- that asks it, never the other way round, for the cells in
//! view.
//!
//! The client holds a [`Host`] and calls it; each call is sent to the
//! host's thread, read there between ticks. It starts with no world,
//! and runs one once asked to make one ([`Host::make_world`]) or to
//! open one saved ([`Host::open_world`]). Each [`Host::sync`] is
//! answered with a [`Frame`]: the superchunks in view as the last tick
//! left them, copied and nothing more ([`frame`]) -- so what is in view
//! costs the ticks next to nothing, and a frame carries only so many
//! superchunks, the client going round those in view. It sends nothing
//! unasked, so it is the client that sets how often the world is
//! drawn, and one that falls behind slows no tick.
//!
//! It ticks at the pace asked, or flat out, until the client is gone,
//! and keeps a census of the flock and the grass as it goes
//! ([`census_path`]): what a long run came to is there once it is
//! closed.

pub mod frame;

use crate::{Start, World};
use chunk_storage::mock::GRASS;
use frame::{copy, count, Ask, Frame};
use mc_rules::trees::TREE;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

/// Ticks a second a world is held to unless told otherwise: the
/// game's target.
pub const TARGET_PACE: u32 = 256;

/// Ticks from one line of the census to the next.
pub const CENSUS_EVERY: u64 = 1000;

/// How far behind its pace the host may fall and still catch up:
/// ticks made late by a frame or a sleep are made up, a stall is not.
const CATCH_UP: Duration = Duration::from_millis(250);

/// What a client asks of the host, sent to its thread by a [`Host`]'s
/// calls.
#[derive(Clone, Debug)]
enum Request {
    /// Some of the superchunks in view: answered with a [`Frame`] --
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
}

impl Host {
    /// Starts the host on a thread of its own, no world run yet: the
    /// host, and where its frames come back. It stops once the host is
    /// dropped.
    pub fn start() -> (Self, Receiver<Frame>) {
        let (requests, asked) = channel();
        let (answers, frames) = channel();
        thread::Builder::new().name("host".to_string()).spawn(move || HostThread::default().run(&asked, &answers)).expect("a thread for the host");
        (Self { requests }, frames)
    }

    /// Asks for some of the superchunks in view: answered with a
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
}

/// The world run, and what goes with running it.
struct Running {
    /// The world.
    world: World,
    /// The superchunks whose heights a frame has carried, each with its
    /// water as drawn.
    sent: HashMap<coordinates::SuperchunkIndex, frame::Water>,
    /// When it began to run.
    started: Instant,
    /// Its census, if one could be kept.
    census: Option<BufWriter<File>>,
}

impl Running {
    /// `world`, starting to run.
    fn of(world: World) -> Self {
        let census = census(world.info.seed);
        Self { world, sent: HashMap::new(), started: Instant::now(), census }
    }
}

/// The host's state between ticks, on its own thread.
struct HostThread {
    /// The world run, if one is.
    running: Option<Running>,
    /// Counts the worlds run.
    worlds: u64,
    /// Whether ticking is paused.
    paused: bool,
    /// Ticks a second it is held to, or flat out.
    pace: Option<u32>,
    /// The name of the world run, if it is one of the worlds' folder.
    named: Option<String>,
    /// What opening or saving a world last came to.
    said: Option<String>,
    /// When the next tick is due.
    next_tick: Instant,
    /// When the last frame was answered, and the tick then.
    last_frame: (Instant, u64),
}

impl Default for HostThread {
    /// No world, at the game's pace.
    fn default() -> Self {
        Self { running: None, worlds: 0, paused: false, pace: Some(TARGET_PACE), named: None, said: None, next_tick: Instant::now(), last_frame: (Instant::now(), 0) }
    }
}

impl HostThread {
    /// Requests read between ticks, a tick, and a wait for the next
    /// one's time -- until the client is gone.
    fn run(mut self, asked: &Receiver<Request>, answers: &Sender<Frame>) {
        loop {
            // Paused, or with no world, there is nothing to do until the client asks.
            let mut request = if self.paused || self.running.is_none() { asked.recv().ok() } else { None };
            loop {
                match request.take().map_or_else(|| asked.try_recv(), Ok) {
                    Ok(Request::Sync(ask)) => {
                        if let Some(frame) = self.frame(ask)
                            && answers.send(frame).is_err()
                        {
                            return;
                        }
                    }
                    Ok(Request::Pause(pause)) => (self.paused, self.next_tick) = (pause, Instant::now()),
                    Ok(Request::Pace(pace)) => (self.pace, self.next_tick) = (pace, Instant::now()),
                    Ok(Request::New(start)) => {
                        self.said = Some(format!("a world made from seed {}", utilities::seed::hex(start.seed)));
                        self.named = None;
                        self.run_world(crate::start(start));
                    }
                    Ok(Request::Open(name)) => match crate::load(&utilities::settings::world(&name)) {
                        Ok(opened) => {
                            (self.said, self.named) = (Some(format!("{name} opened")), Some(name));
                            self.run_world(opened);
                        }
                        Err(why) => self.said = Some(format!("{name} not opened: {why}")),
                    },
                    Ok(Request::Save(name)) => self.save(name),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if !self.paused {
                self.tick();
            }
        }
    }

    /// Runs `world` in place of any run, from its own tick.
    fn run_world(&mut self, world: World) {
        self.last_frame = (Instant::now(), world.entities.now());
        self.running = Some(Running::of(world));
        self.worlds += 1;
        self.next_tick = Instant::now();
    }

    /// Saves the world run under `name`, if one runs.
    fn save(&mut self, name: String) {
        let Some(running) = &mut self.running else {
            return;
        };
        let world = &mut running.world;
        self.said = Some(match crate::save(&utilities::settings::world(&name), world) {
            Ok(saved) => {
                let said = format!("{name} saved at tick {}: {} superchunks, {} entities", world.entities.now(), saved.superchunks, saved.entities);
                self.named = Some(name);
                said
            }
            Err(why) => format!("{name} not saved: {why}"),
        });
        self.next_tick = Instant::now();
    }

    /// The frame `ask` asks for, if a world runs -- its view kept hot
    /// from then, if the world's camera loads superchunks.
    fn frame(&mut self, ask: Ask) -> Option<Frame> {
        let running = self.running.as_mut()?;
        running.world.keep_in_view(ask.viewport);
        let world = &running.world;
        let asked_at = Instant::now();
        let (tick, elapsed) = (world.entities.now(), self.last_frame.0.elapsed().as_secs_f64());
        let ticks_a_second = if elapsed > 0.0 { (tick - self.last_frame.1) as f64 / elapsed } else { 0.0 };
        self.last_frame = (asked_at, tick);
        let (sheep, grass, trees, cells) = (world.entities.len(), count(world, GRASS), count(world, TREE), copy(world, ask, &mut running.sent));
        let sync_seconds = asked_at.elapsed().as_secs_f64();
        let sync_share = if elapsed > 0.0 { sync_seconds / elapsed } else { 0.0 };
        let (named, said) = (self.named.clone(), self.said.clone());
        Some(Frame { world: self.worlds, seed: world.info.seed, generation: world.generation, side: world.halos.hot.side(), tick, ticks_a_second, sheep, grass, trees, sync_seconds, sync_share, detail: ask.detail, near: ask.near, named, said, cells })
    }

    /// A tick of the world run, its census written when due, and a wait
    /// for the next one's time at the pace asked.
    fn tick(&mut self) {
        let Some(running) = &mut self.running else {
            return;
        };
        let tick = running.world.entities.now();
        if tick.is_multiple_of(CENSUS_EVERY)
            && let Some(file) = &mut running.census
        {
            // A line lost is a line lost: the run goes on. The pace it is held to, 0 flat out: only flat out do the seconds say what a tick costs.
            let (seconds, sheep, grass) = (running.started.elapsed().as_secs_f64(), running.world.entities.len(), count(&running.world, GRASS));
            _ = writeln!(file, "{tick},{seconds:.3},{},{sheep},{grass}", self.pace.unwrap_or(0)).and_then(|()| file.flush());
        }
        running.world.tick();
        if let Some(pace) = self.pace {
            self.next_tick += Duration::from_secs_f64(1.0 / pace as f64);
            let now = Instant::now();
            if self.next_tick > now {
                thread::sleep(self.next_tick - now);
            } else if now - self.next_tick > CATCH_UP {
                // Far behind: no catching up in a long burst.
                self.next_tick = now;
            }
        }
    }
}
