//! The host's own thread: the world run, requests read between its
//! ticks, frames answered.

use crate::{Start, World};
use super::{CATCH_UP, CENSUS_EVERY, FRAMES_SHARE, Request, TARGET_PACE, census};
use super::frame::{self, Ask, Cells, Frame, copy, count, hot_in};
use super::terrain::{Levels, TerrainRequest};
use type_registry::GRASS;
use type_registry::TREE;
use std::thread;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use worldgen::Generation;

/// The world run, and what goes with running it.
struct Running {
    /// The world.
    world: World,
    /// What it started from, as far as is known: what it is made again
    /// from when reset.
    start: Start,
    /// The superchunks whose heights a frame has carried, each with its
    /// water as drawn.
    sent: HashMap<coordinates::SuperchunkIndex, Arc<frame::Water>>,
    /// When it began to run.
    started: Instant,
    /// Its census, if one could be kept.
    census: Option<BufWriter<File>>,
}

impl Running {
    /// `world`, started from `start`, starting to run.
    fn of(start: Start, world: World) -> Self {
        let census = census(world.info.seed);
        Self { world, start, sent: HashMap::new(), started: Instant::now(), census }
    }
}

/// An ask being answered: what was asked, and what is yet to copy --
/// a few superchunks between two ticks, each few sent as a frame as
/// soon as they are copied, the last saying it is ([`Frame::more`]).
/// Seen from near they are one picture, so sent the once, together.
struct Answering {
    /// What was asked.
    ask: Ask,
    /// The viewport's hot superchunks when it was asked.
    hot: Vec<(u32, u32)>,
    /// Those of them asked for and yet to copy, the next one last.
    to_copy: Vec<(u32, u32)>,
    /// Those copied and not yet sent.
    cells: Vec<Cells>,
    /// What copying has taken of the host's thread so far.
    spent: Duration,
}

/// The host's state between ticks, on its own thread.
pub(crate) struct HostThread {
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
    /// The generation the world run is to be made again with, before
    /// the next tick, if it is to be.
    reset: Option<Generation>,
    /// When the next tick is due.
    next_tick: Instant,
    /// When the last frame was answered, and the tick then.
    last_frame: (Instant, u64),
    /// The ask being answered, if one is.
    answering: Option<Answering>,
    /// What the last ask answered whole came to: the ticks a second
    /// since the one before, the seconds answering took the host's
    /// thread, and the share of its time that is.
    measured: (f64, f64, f64),
    /// What the last tick took.
    tick_took: Duration,
    /// The terrain's thread, told of each world run.
    terrain: Sender<TerrainRequest>,
}

impl HostThread {
    /// No world, at the game's pace; `terrain` told of each world run.
    pub(crate) fn telling(terrain: Sender<TerrainRequest>) -> Self {
        Self { terrain, running: None, worlds: 0, paused: false, pace: Some(TARGET_PACE), named: None, said: None, reset: None, next_tick: Instant::now(), last_frame: (Instant::now(), 0), answering: None, measured: (0.0, 0.0, 0.0), tick_took: Duration::ZERO }
    }
}

impl HostThread {
    /// Requests read between ticks, a tick, and a wait for the next
    /// one's time -- until the client is gone.
    pub(crate) fn run(mut self, asked: &Receiver<Request>, answers: &Sender<Frame>) {
        loop {
            // Paused, or with no world, there is nothing to do until the client asks -- once what it asked is answered.
            let mut request = if (self.paused || self.running.is_none()) && self.answering.is_none() { asked.recv().ok() } else { None };
            loop {
                match request.take().map_or_else(|| asked.try_recv(), Ok) {
                    Ok(Request::Sync(ask)) => {
                        // A frame asked after a reset is of the world remade.
                        self.reset_now();
                        self.begin_frame(ask);
                    }
                    Ok(Request::Pause(pause)) => (self.paused, self.next_tick) = (pause, Instant::now()),
                    Ok(Request::Pace(pace)) => (self.pace, self.next_tick) = (pace, Instant::now()),
                    Ok(Request::New(start)) => {
                        self.said = Some(format!("a world made from seed {}", utilities::seed::hex(start.seed)));
                        (self.named, self.reset) = (None, None);
                        self.run_world(start);
                    }
                    Ok(Request::Open(name)) => match crate::load(&utilities::settings::world(&name)) {
                        Ok(opened) => {
                            (self.said, self.named, self.reset) = (Some(format!("{name} opened")), Some(name), None);
                            self.run_in_place(Running::of(Start::of_world(&opened.info, opened.generation), opened));
                        }
                        Err(why) => self.said = Some(format!("{name} not opened: {why}")),
                    },
                    Ok(Request::Save(name)) => {
                        self.reset_now();
                        self.save(name);
                    }
                    Ok(Request::Reset(generation)) => self.reset = Some(*generation),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            self.reset_now();
            if let Some(frame) = self.answer_a_little()
                && answers.send(frame).is_err()
            {
                return;
            }
            if !self.paused {
                self.tick();
            }
        }
    }

    /// Makes a world as `start` says, and runs it in place of any run:
    /// that one dropped first, not to hold both.
    fn run_world(&mut self, start: Start) {
        self.running = None;
        self.run_in_place(Running::of(start, crate::start(start)));
    }

    /// Runs `running` in place of any run, from its world's own tick.
    fn run_in_place(&mut self, running: Running) {
        self.last_frame = (Instant::now(), running.world.entities.now());
        // A frame half answered was of the world before.
        self.answering = None;
        self.worlds += 1;
        // Told before any frame of this world is sent: an ask of it finds it known. The thread gone, there is no one to ask either.
        _ = self.terrain.send(TerrainRequest::World(self.worlds, running.world.info.seed, Box::new(running.world.generation)));
        self.running = Some(running);
        self.next_tick = Instant::now();
    }

    /// Makes the world run again from its start, generated as last
    /// asked, if it was asked to be: a world of its own from then, of
    /// no name.
    fn reset_now(&mut self) {
        let (Some(generation), Some(running)) = (self.reset.take(), &self.running) else {
            return;
        };
        let start = Start { generation, ..running.start };
        (self.said, self.named) = (Some(format!("the world remade from seed {}, as the sliders have it", utilities::seed::hex(start.seed))), None);
        self.run_world(start);
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

    /// Begins answering `ask`, if a world runs, in place of any frame
    /// half answered -- its viewport kept hot from now, if the world's
    /// camera loads superchunks.
    fn begin_frame(&mut self, ask: Ask) {
        let Some(running) = &mut self.running else {
            return;
        };
        running.world.halos.keep_viewport(ask.viewport);
        let hot = hot_in(&running.world, ask.viewport);
        let mut to_copy: Vec<(u32, u32)> = hot.iter().copied().skip(ask.skip as usize).take(ask.most as usize).collect();
        to_copy.reverse();
        self.answering = Some(Answering { ask, hot, to_copy, cells: Vec::new(), spent: Duration::ZERO });
    }

    /// Copies a little more of what is asked -- a superchunk at
    /// least, and as many as the time to spare takes -- and gives them
    /// as a frame (`docs/server.md`, "The host").
    fn answer_a_little(&mut self) -> Option<Frame> {
        let until = self.time_for_frames().map(|time| Instant::now() + time);
        let (running, answering) = (self.running.as_mut()?, self.answering.as_mut()?);
        let began = Instant::now();
        while let Some(at) = answering.to_copy.pop() {
            answering.cells.extend(copy(&running.world, at, answering.ask, &mut running.sent));
            if until.is_some_and(|until| Instant::now() >= until) {
                break;
            }
        }
        answering.spent += began.elapsed();
        let more = !answering.to_copy.is_empty();
        if more && answering.ask.near.is_some() {
            return None;
        }
        let (ask, hot, cells) = (answering.ask, answering.hot.clone(), std::mem::take(&mut answering.cells));
        let world = &running.world;
        let tick = world.entities.now();
        if !more {
            let (spent, elapsed) = (answering.spent.as_secs_f64(), self.last_frame.0.elapsed().as_secs_f64());
            if elapsed > 0.0 {
                self.measured = ((tick - self.last_frame.1) as f64 / elapsed, spent, spent / elapsed);
            }
            (self.last_frame, self.answering) = ((Instant::now(), tick), None);
        }
        let (ticks_a_second, sync_seconds, sync_share) = self.measured;
        let (sheep, grass, trees) = (world.entities.len(), count(world, GRASS), count(world, TREE));
        let (named, said) = (self.named.clone(), self.said.clone());
        Some(Frame { world: self.worlds, seed: world.info.seed, generation: world.generation, levels: Levels::of(&world.generation), side: world.halos.hot.side(), tick, ticks_a_second, sheep, grass, trees, sync_seconds, sync_share, viewport: ask.viewport, hot, detail: ask.detail, near: ask.near, named, said, more, cells })
    }

    /// The time there is for copying a frame's superchunks before the
    /// next tick: none to keep to if the world is paused -- all of the
    /// frame at once -- else what is left until the tick is due, and
    /// [`FRAMES_SHARE`] of what the last tick took at least.
    fn time_for_frames(&self) -> Option<Duration> {
        if self.paused {
            return None;
        }
        let spare = if self.pace.is_some() { self.next_tick.saturating_duration_since(Instant::now()) } else { Duration::ZERO };
        Some(spare.max(self.tick_took.mul_f64(FRAMES_SHARE)))
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
        let began = Instant::now();
        running.world.tick();
        self.tick_took = began.elapsed();
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
