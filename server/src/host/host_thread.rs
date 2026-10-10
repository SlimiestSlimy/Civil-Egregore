//! The host's own thread: the world run, requests read between its
//! ticks, frames answered.

use crate::{Start, World};
use super::{CATCH_UP, CENSUS_EVERY, Request, TARGET_PACE, census};
use super::frame::{self, Ask, Frame, copy, count, hot_in};
use worldgen::GRASS;
use mc_rules::trees::TREE;
use std::thread;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
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
    sent: HashMap<coordinates::SuperchunkIndex, frame::Water>,
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
}

impl Default for HostThread {
    /// No world, at the game's pace.
    fn default() -> Self {
        Self { running: None, worlds: 0, paused: false, pace: Some(TARGET_PACE), named: None, said: None, reset: None, next_tick: Instant::now(), last_frame: (Instant::now(), 0) }
    }
}

impl HostThread {
    /// Requests read between ticks, a tick, and a wait for the next
    /// one's time -- until the client is gone.
    pub(crate) fn run(mut self, asked: &Receiver<Request>, answers: &Sender<Frame>) {
        loop {
            // Paused, or with no world, there is nothing to do until the client asks.
            let mut request = if self.paused || self.running.is_none() { asked.recv().ok() } else { None };
            loop {
                match request.take().map_or_else(|| asked.try_recv(), Ok) {
                    Ok(Request::Sync(ask)) => {
                        // A frame asked after a reset is of the world remade.
                        self.reset_now();
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
        self.running = Some(running);
        self.worlds += 1;
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

    /// The frame `ask` asks for, if a world runs -- its viewport kept
    /// hot from then, if the world's camera loads superchunks.
    fn frame(&mut self, ask: Ask) -> Option<Frame> {
        let running = self.running.as_mut()?;
        running.world.halos.keep_viewport(ask.viewport);
        let world = &running.world;
        let asked_at = Instant::now();
        let (tick, elapsed) = (world.entities.now(), self.last_frame.0.elapsed().as_secs_f64());
        let ticks_a_second = if elapsed > 0.0 { (tick - self.last_frame.1) as f64 / elapsed } else { 0.0 };
        self.last_frame = (asked_at, tick);
        let hot = hot_in(world, ask.viewport);
        let (sheep, grass, trees, cells) = (world.entities.len(), count(world, GRASS), count(world, TREE), copy(world, &hot, ask, &mut running.sent));
        let sync_seconds = asked_at.elapsed().as_secs_f64();
        let sync_share = if elapsed > 0.0 { sync_seconds / elapsed } else { 0.0 };
        let (named, said) = (self.named.clone(), self.said.clone());
        Some(Frame { world: self.worlds, seed: world.info.seed, generation: world.generation, side: world.halos.hot.side(), tick, ticks_a_second, sheep, grass, trees, sync_seconds, sync_share, viewport: ask.viewport, hot, detail: ask.detail, near: ask.near, named, said, cells })
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
