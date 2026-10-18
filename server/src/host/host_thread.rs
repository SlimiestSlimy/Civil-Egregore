//! The host's own thread: the world run, requests read between its
//! ticks, frames answered.

use crate::{WorldStart, World};
use super::{CATCH_UP, FRAMES_SHARE, HostRequest, HostSent, TARGET_PACE};
use super::frame::{copy_superchunk, frame_of, hot_in};
use protocol::pipe::frame::{Cells, FrameAsk, ViewportFrame};
use super::terrain::{TerrainRequest, WorldTold};
use std::thread;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};
use std::sync::Arc;
use protocol::type_registry::FrozenTypes;
use protocol::world_types::GameTypes;
use utilities::tuning::Tuning;

/// The world run, and what goes with running it.
struct RunningWorld {
    /// The world.
    world: World,
    /// What it started from, as far as is known: what it is made again
    /// from when reset.
    world_start: WorldStart,
}

impl RunningWorld {
    /// `world`, started from `world_start`, starting to run.
    fn from_start(world_start: WorldStart, world: World) -> Self {
        Self { world, world_start }
    }
}

/// An ask being answered: what was asked, and what is yet to copy --
/// a few superchunks between two ticks, each few sent as a frame as
/// soon as they are copied, the last saying it is ([`Frame::more`]).
/// Seen from near they are one picture, so sent the once, together.
struct AskBeingAnswered {
    /// What was asked.
    frame_ask: FrameAsk,
    /// The viewport's hot superchunks when it was asked.
    hot: Vec<(u32, u32)>,
    /// Those of them asked for and yet to copy, the next one last.
    to_copy: Vec<(u32, u32)>,
    /// Those copied and not yet sent.
    cells: Vec<Cells>,
    /// What copying has taken of the host's thread so far.
    copying_spent: Duration,
}

/// The host's state between ticks, on its own thread.
pub(crate) struct HostThread {
    /// The world run, if one is.
    running_world: Option<RunningWorld>,
    /// Counts the worlds run.
    worlds: u64,
    /// Whether ticking is paused.
    ticking_paused: bool,
    /// Ticks a second it is held to, or flat out.
    pace_held_to: Option<u32>,
    /// The name of the world run, if it is one of the worlds' folder.
    world_name: Option<String>,
    /// What opening or saving a world last came to.
    last_outcome: Option<String>,
    /// The sliders the world run is to be made again as, before the
    /// next tick, if it is to be.
    reset: Option<Tuning>,
    /// When the next tick is due.
    next_tick: Instant,
    /// When the last frame was answered, and the tick then.
    last_frame: (Instant, u64),
    /// The ask being answered, if one is.
    being_answered: Option<AskBeingAnswered>,
    /// What the last ask answered whole came to: the ticks a second
    /// since the one before, the seconds answering took the host's
    /// thread, and the share of its time that is.
    last_answer_measured: (f64, f64, f64),
    /// What the last tick took.
    tick_took: Duration,
    /// The terrain's thread, told of each world run.
    terrain: Sender<TerrainRequest>,
    /// What registers the types of a world opened.
    game_types: GameTypes,
}

impl HostThread {
    /// No world, at the game's pace; `terrain` told of each world run;
    /// worlds opened of `game_types`.
    pub(crate) fn telling_terrain(terrain: Sender<TerrainRequest>, game_types: GameTypes) -> Self {
        Self { terrain, game_types, running_world: None, worlds: 0, ticking_paused: false, pace_held_to: Some(TARGET_PACE), world_name: None, last_outcome: None, reset: None, next_tick: Instant::now(), last_frame: (Instant::now(), 0), being_answered: None, last_answer_measured: (0.0, 0.0, 0.0), tick_took: Duration::ZERO }
    }
}

impl HostThread {
    /// Requests read between ticks, a tick, and a wait for the next
    /// one's time -- until the client is gone. Each world run, its types
    /// are sent once, before any frame of it.
    pub(crate) fn serve_requests(mut self, request_receiver: &Receiver<HostRequest>, sender: &Sender<HostSent>) {
        let mut worlds_told = 0;
        loop {
            // Paused, or with no world, there is nothing to do until the client asks -- once what it asked is answered.
            let mut waited_request = if (self.ticking_paused || self.running_world.is_none()) && self.being_answered.is_none() { request_receiver.recv().ok() } else { None };
            loop {
                match waited_request.take().map_or_else(|| request_receiver.try_recv(), Ok) {
                    Ok(HostRequest::AskFrame(frame_ask)) => {
                        // A frame asked after a reset is of the world remade.
                        self.reset_now();
                        self.begin_frame(frame_ask);
                    }
                    Ok(HostRequest::SetPaused(pause_asked)) => (self.ticking_paused, self.next_tick) = (pause_asked, Instant::now()),
                    Ok(HostRequest::SetPace(pace_asked)) => (self.pace_held_to, self.next_tick) = (pace_asked, Instant::now()),
                    Ok(HostRequest::MakeWorld(world_start, frozen_types)) => {
                        self.last_outcome = Some(format!("a world made from seed {}", utilities::seed::seed_as_hex(world_start.seed)));
                        (self.world_name, self.reset) = (None, None);
                        self.run_world(world_start, &frozen_types);
                    }
                    Ok(HostRequest::OpenWorld(world_to_open)) => match crate::load_world(&utilities::settings::world(&world_to_open), self.game_types) {
                        Ok(opened_world) => {
                            (self.last_outcome, self.world_name, self.reset) = (Some(format!("{world_to_open} opened")), Some(world_to_open), None);
                            self.run_in_place(RunningWorld::from_start(WorldStart::of_world(&opened_world.world_info), opened_world));
                        }
                        Err(load_error) => self.last_outcome = Some(format!("{world_to_open} not opened: {load_error}")),
                    },
                    Ok(HostRequest::Save(name_to_save)) => {
                        self.reset_now();
                        self.save(name_to_save);
                    }
                    Ok(HostRequest::Reset(reset_tuning)) => self.reset = Some(*reset_tuning),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            self.reset_now();
            if let Some(running_world) = &self.running_world
                && worlds_told != self.worlds
            {
                worlds_told = self.worlds;
                if sender.send(HostSent::WorldRuns(self.worlds, Arc::clone(&running_world.world.frozen_types))).is_err() {
                    return;
                }
            }
            if let Some(answered_frame) = self.answer_a_little()
                && sender.send(HostSent::Frame(Box::new(answered_frame))).is_err()
            {
                return;
            }
            if !self.ticking_paused {
                self.tick();
            }
        }
    }

    /// Makes a world as `world_start` says, of `frozen_types`, and runs it in
    /// place of any run: that one dropped first, not to hold both.
    fn run_world(&mut self, world_start: WorldStart, frozen_types: &Arc<FrozenTypes>) {
        self.running_world = None;
        let world = crate::start_world(world_start.clone(), frozen_types);
        self.run_in_place(RunningWorld::from_start(world_start, world));
    }

    /// Runs `running_world` in place of any run, from its world's own tick.
    fn run_in_place(&mut self, running_world: RunningWorld) {
        self.last_frame = (Instant::now(), running_world.world.entity_store().current_tick());
        // A frame half answered was of the world before.
        self.being_answered = None;
        self.worlds += 1;
        // Told before any frame of this world is sent: an ask of it finds it known. The thread gone, there is no one to ask either.
        let world = &running_world.world;
        _ = self.terrain.send(TerrainRequest::World(WorldTold { world: self.worlds, seed: world.world_info.seed, generation_numbers: world.world_info.generation_numbers.clone(), worldgen: world.worldgen.clone() }));
        self.running_world = Some(running_world);
        self.next_tick = Instant::now();
    }

    /// Makes the world run again from its start, generated as the
    /// sliders last asked say, if it was asked to be: a world of its own
    /// from then, of no name.
    fn reset_now(&mut self) {
        let (Some(reset_tuning), Some(running_world)) = (self.reset.take(), &self.running_world) else {
            return;
        };
        // Remade of its own types: its IDs stay.
        let (world_start, frozen_types) = (WorldStart { generation_numbers: (running_world.world.worldgen.numbers_of_tuning)(&reset_tuning), ..running_world.world_start.clone() }, Arc::clone(&running_world.world.frozen_types));
        (self.last_outcome, self.world_name) = (Some(format!("the world remade from seed {}, as the sliders have it", utilities::seed::seed_as_hex(world_start.seed))), None);
        self.run_world(world_start, &frozen_types);
    }

    /// Saves the world run under `world_name`, if one runs.
    fn save(&mut self, world_name: String) {
        let Some(running_world) = &mut self.running_world else {
            return;
        };
        let world = &mut running_world.world;
        self.last_outcome = Some(match crate::save(&utilities::settings::world(&world_name), world) {
            Ok(save_written) => {
                let saved_text = format!("{world_name} saved at tick {}: {} superchunks, {} entities", world.entity_store().current_tick(), save_written.superchunks, save_written.entities_written);
                self.world_name = Some(world_name);
                saved_text
            }
            Err(save_error) => format!("{world_name} not saved: {save_error}"),
        });
        self.next_tick = Instant::now();
    }

    /// Begins answering `frame_ask`, if a world runs, in place of any frame
    /// half answered -- its viewport kept hot from now, if the world's
    /// camera loads superchunks.
    fn begin_frame(&mut self, frame_ask: FrameAsk) {
        let Some(running_world) = &mut self.running_world else {
            return;
        };
        running_world.world.keep_viewport(frame_ask.viewport);
        let hot = hot_in(&running_world.world, frame_ask.viewport);
        let mut to_copy: Vec<(u32, u32)> = hot.iter().copied().skip(frame_ask.superchunks_skipped as usize).take(frame_ask.most_superchunks as usize).collect();
        to_copy.reverse();
        self.being_answered = Some(AskBeingAnswered { frame_ask, hot, to_copy, cells: Vec::new(), copying_spent: Duration::ZERO });
    }

    /// Copies a little more of what is asked -- a superchunk at
    /// least, and as many as the time to spare takes -- and gives them
    /// as a frame (`docs/server.md`, "The host").
    fn answer_a_little(&mut self) -> Option<ViewportFrame> {
        let copy_until = self.time_for_frames().map(|time_for_copying| Instant::now() + time_for_copying);
        let (running_world, being_answered) = (self.running_world.as_mut()?, self.being_answered.as_mut()?);
        let copying_began = Instant::now();
        while let Some(superchunk_cartesian) = being_answered.to_copy.pop() {
            being_answered.cells.extend(copy_superchunk(&running_world.world, superchunk_cartesian, &being_answered.frame_ask));
            if copy_until.is_some_and(|until_instant| Instant::now() >= until_instant) {
                break;
            }
        }
        being_answered.copying_spent += copying_began.elapsed();
        let more_to_copy = !being_answered.to_copy.is_empty();
        if more_to_copy && being_answered.frame_ask.seen_near.is_some() {
            return None;
        }
        let (frame_ask, hot, cells) = (being_answered.frame_ask.clone(), being_answered.hot.clone(), std::mem::take(&mut being_answered.cells));
        let world = &running_world.world;
        let tick = world.entity_store().current_tick();
        if !more_to_copy {
            let (copying_seconds, seconds_since_frame) = (being_answered.copying_spent.as_secs_f64(), self.last_frame.0.elapsed().as_secs_f64());
            if seconds_since_frame > 0.0 {
                self.last_answer_measured = ((tick - self.last_frame.1) as f64 / seconds_since_frame, copying_seconds, copying_seconds / seconds_since_frame);
            }
            (self.last_frame, self.being_answered) = ((Instant::now(), tick), None);
        }
        Some(frame_of(world, self.worlds, &frame_ask, hot, cells, self.last_answer_measured, (self.world_name.clone(), self.last_outcome.clone()), more_to_copy))
    }

    /// The time there is for copying a frame's superchunks before the
    /// next tick: none to keep to if the world is paused -- all of the
    /// frame at once -- else what is left until the tick is due, and
    /// [`FRAMES_SHARE`] of what the last tick took at least.
    fn time_for_frames(&self) -> Option<Duration> {
        if self.ticking_paused {
            return None;
        }
        let time_to_spare = if self.pace_held_to.is_some() { self.next_tick.saturating_duration_since(Instant::now()) } else { Duration::ZERO };
        Some(time_to_spare.max(self.tick_took.mul_f64(FRAMES_SHARE)))
    }

    /// A tick of the world run, and a wait
    /// for the next one's time at the pace asked.
    fn tick(&mut self) {
        let Some(running_world) = &mut self.running_world else {
            return;
        };
        let tick_began = Instant::now();
        running_world.world.tick();
        self.tick_took = tick_began.elapsed();
        if let Some(ticks_a_second) = self.pace_held_to {
            self.next_tick += Duration::from_secs_f64(1.0 / ticks_a_second as f64);
            let time_now = Instant::now();
            if self.next_tick > time_now {
                thread::sleep(self.next_tick - time_now);
            } else if time_now - self.next_tick > CATCH_UP {
                // Far behind: no catching up in a long burst.
                self.next_tick = time_now;
            }
        }
    }
}
