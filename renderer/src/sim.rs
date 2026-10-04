//! The simulation's side: a pasture ticked on a thread of its own, which
//! the window asks -- never the other way round -- for the cells in
//! view.
//!
//! The window sends [`Request`]s; the simulation reads them between
//! ticks, and answers each [`Request::Sync`] with a [`Frame`]: the
//! superchunks in view as the last tick left them -- their grass's
//! words, copied as they are, and where their sheep stand. It copies and
//! nothing more: turning cells into pixels is [`crate::paint`]'s, on
//! another thread, so what is in view costs the ticks next to nothing
//! -- and a frame carries only so many superchunks, the window going
//! round those in view, so it costs no more however many there are.
//! It sends nothing unasked, so it is the window that sets how often
//! the world is drawn, and a window that falls behind slows no tick.
//!
//! It ticks until the window is closed, with no number of ticks to
//! stop at, and keeps a census of the flock and the grass as it goes
//! ([`census_path`]): what a long run came to is there once it is
//! closed.

use bitplane_manager::BucketKey;
use chunk_storage::mock::GRASS;
use chunk_storage::LayerType;
use coordinates::{square_side, CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use world::World;
use utilities::rng::Rng;

/// Ticks a second the simulation is held to unless told otherwise: the
/// game's target.
pub const TARGET_PACE: u32 = 256;

/// The seed of the world watched.
pub const SEED: u64 = 1;

/// Ticks from one line of the census to the next.
pub const CENSUS_EVERY: u64 = 1000;

/// Where the census of the run is kept: the flock and the grass every
/// [`CENSUS_EVERY`] ticks, written as the run goes, so a run closed at
/// any time leaves what it came to. Under the crate's folder, out of
/// git, as every crate's transient data.
pub fn census_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("transient_data/measurements/census.csv")
}

/// Starts the census afresh: its file, with what was run and the
/// columns' names. `None`, and no census kept, if it cannot be made.
fn census(superchunks: u32, flock: usize, forced_hot: bool) -> Option<BufWriter<File>> {
    let path = census_path();
    create_dir_all(path.parent()?).ok()?;
    let mut file = BufWriter::new(File::create(path).ok()?);
    writeln!(file, "# renderer {superchunks} {flock}{}", if forced_hot { " forced hot" } else { "" }).ok()?;
    writeln!(file, "tick,seconds,pace,sheep,grass").ok()?;
    Some(file)
}

/// Words a chunk's bitmap takes.
pub const CHUNK_WORDS: usize = bitmap::WORDS;

/// The superchunks in view: a rectangle of them, each `(x, y)` in
/// superchunks from the world's top left, both corners in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// The top left superchunk, `(x, y)`.
    pub first: (u32, u32),
    /// The bottom right one.
    pub last: (u32, u32),
}

/// What the window wants of the world, one frame: some of the
/// superchunks in view -- as many as a frame may carry, the window
/// going round them frame after frame -- and how finely it will draw
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The superchunks in view.
    pub viewport: Viewport,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    /// Passed on to the painter; the simulation copies the same.
    pub detail: u32,
    /// Superchunks of the view, row by row, to pass over first.
    pub skip: u32,
    /// Superchunks to answer with, at most.
    pub most: u32,
    /// The cells in view, if they are seen from near: drawn as one
    /// picture, a cell many pixels. Passed on to the painter.
    pub near: Option<Near>,
}

/// Cells seen from near: a rectangle of them, each many pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Near {
    /// The top left cell, `(x, y)` in the world.
    pub first: (u32, u32),
    /// Cells across and down.
    pub size: (u32, u32),
    /// Pixels along a cell's side: 2, 4 or 8.
    pub pixels_a_cell: u32,
}

impl Ask {
    /// The superchunks it asks for, each `(x, y)` in the world: those
    /// of the view, row by row, after the ones passed over.
    pub fn asked(self) -> impl Iterator<Item = (u32, u32)> {
        let (first, last) = (self.viewport.first, self.viewport.last);
        let in_view = (first.1..=last.1.min(WORLD_SIDE_SUPERCHUNKS - 1)).flat_map(move |y| (first.0..=last.0.min(WORLD_SIDE_SUPERCHUNKS - 1)).map(move |x| (x, y)));
        in_view.skip(self.skip as usize).take(self.most as usize)
    }
}

/// What the window asks of the simulation.
#[derive(Clone, Copy, Debug)]
pub enum Request {
    /// Some of the superchunks in view: answered with a [`Frame`].
    Sync(Ask),
    /// Stop ticking, or go on.
    Pause(bool),
    /// Tick so many times a second, or flat out.
    Pace(Option<u32>),
}

/// One superchunk's cells, as a tick left them.
pub struct Cells {
    /// Where it is in the world, `(x, y)` in superchunks.
    pub at: (u32, u32),
    /// Whether it is hot: cold, it has no cells here, and is drawn dark.
    pub hot: bool,
    /// Its grass: its 16 chunks' bitmaps one after another, in the
    /// chunks' Morton order, [`CHUNK_WORDS`] words each, in Morton order
    /// -- as the arena holds them. A chunk not hot is all clear.
    pub grass: Vec<u64>,
    /// Its top left cell in the world, `(x, y)`: what its ground is
    /// worked out from.
    pub top_left: (u32, u32),
    /// The cells its sheep stand on, `(x, y)` from its top left.
    pub sheep: Vec<(u16, u16)>,
}

/// The world in view, as a tick left it.
pub struct Frame {
    /// Ticks run so far.
    pub tick: u64,
    /// Ticks a second, over the time since the frame before.
    pub ticks_a_second: f64,
    /// Sheep in the whole world.
    pub sheep: usize,
    /// Cells of grass in the whole world.
    pub grass: u64,
    /// What answering took of the simulation's thread -- the counts and
    /// the copy, all the window costs it -- in seconds.
    pub sync_seconds: f64,
    /// The share of the thread's time that is, at the rate asked.
    pub sync_share: f64,
    /// How coarsely the window will draw them ([`Ask::detail`]).
    pub detail: u32,
    /// The cells seen from near ([`Ask::near`]).
    pub near: Option<Near>,
    /// How many times how the world is generated had changed
    /// ([`crate::tuning::generation`]): the ground made under an
    /// earlier count is made again.
    pub generation: u64,
    /// The superchunks asked for.
    pub cells: Vec<Cells>,
}

/// Starts a world generated from [`SEED`], `superchunks` of them shown
/// about its origin, `flock` sheep on each -- ticking on
/// every thread the machine has, on a thread of its own: where to send it requests,
/// and where its frames come back. It stops once the requests' sender is
/// dropped.
pub fn start(superchunks: u32, flock: usize, forced_hot: bool) -> (Sender<Request>, Receiver<Frame>) {
    let (requests, asked) = channel();
    let (answers, frames) = channel();
    thread::Builder::new()
        .name("simulation".to_string())
        .spawn(move || run(superchunks, flock, forced_hot, &asked, &answers))
        .expect("a thread for the simulation");
    (requests, frames)
}

/// The simulation's thread: requests read between ticks, a tick, and a
/// wait for the next one's time.
fn run(superchunks: u32, flock: usize, forced_hot: bool, asked: &Receiver<Request>, answers: &Sender<Frame>) {
    let shown = shown(superchunks);
    let mut world = if forced_hot { forced(&shown, flock) } else { world::generate_flocks(SEED, &shown, flock) };
    let started = Instant::now();
    let (mut paused, mut pace, mut tick) = (false, Some(TARGET_PACE), 0u64);
    let mut census = census(superchunks, flock, forced_hot);
    let (mut next_tick, mut last_frame, mut last_frame_tick) = (Instant::now(), Instant::now(), 0u64);
    loop {
        // Paused, there is nothing to do until the window asks.
        let mut request = if paused { asked.recv().ok() } else { None };
        loop {
            match request.take().map_or_else(|| asked.try_recv(), Ok) {
                Ok(Request::Sync(ask)) => {
                    let asked_at = Instant::now();
                    let elapsed = last_frame.elapsed().as_secs_f64();
                    let ticks_a_second = if elapsed > 0.0 { (tick - last_frame_tick) as f64 / elapsed } else { 0.0 };
                    (last_frame, last_frame_tick) = (asked_at, tick);
                    let (sheep, grass, cells) = (world.entities.len(), grass(&world), copy(&world, ask));
                    let sync_seconds = asked_at.elapsed().as_secs_f64();
                    let sync_share = if elapsed > 0.0 { sync_seconds / elapsed } else { 0.0 };
                    let frame = Frame { tick, ticks_a_second, sheep, grass, sync_seconds, sync_share, detail: ask.detail, near: ask.near, generation: crate::tuning::generation(), cells };
                    if answers.send(frame).is_err() {
                        return;
                    }
                }
                Ok(Request::Pause(pause)) => (paused, next_tick) = (pause, Instant::now()),
                Ok(Request::Pace(new)) => (pace, next_tick) = (new, Instant::now()),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if paused {
            continue;
        }
        if tick.is_multiple_of(CENSUS_EVERY) {
            if let Some(file) = &mut census {
                // A line lost is a line lost: the run goes on.
                // The pace it is held to, 0 flat out: only flat out do the seconds say what a tick costs.
                _ = writeln!(file, "{tick},{:.3},{},{},{}", started.elapsed().as_secs_f64(), pace.unwrap_or(0), world.entities.len(), grass(&world)).and_then(|()| file.flush());
            }
        }
        if forced_hot {
            // The halos are not moved: every superchunk shown stays hot.
            world::tick_rules(&mut world.simulation, &mut world.arena, &mut world.entities, SEED);
        } else {
            world.tick();
        }
        tick += 1;
        if let Some(pace) = pace {
            next_tick += Duration::from_secs_f64(1.0 / pace as f64);
            let now = Instant::now();
            if next_tick > now {
                thread::sleep(next_tick - now);
            } else {
                // Behind: no catching up in a burst.
                next_tick = now;
            }
        }
    }
}

/// The superchunks shown: `superchunks` of them in a square, row by row,
/// the world's origin superchunk ([`WORLD_MIDDLE`]) in its middle.
pub fn shown(superchunks: u32) -> Vec<SuperchunkIndex> {
    let (side, (x, y)) = (square_side(superchunks), WORLD_MIDDLE.cartesian());
    let (left, top) = (x - side / 2, y - side / 2);
    (0..side * side).map(|index| SuperchunkIndex::from_cartesian(left + index % side, top + index / side)).collect()
}

/// A world to measure under full load: every superchunk of `shown`
/// hot and kept so, a flock of `flock` sheep on each.
fn forced(shown: &[SuperchunkIndex], flock: usize) -> World {
    let mut world = world::generate(SEED, 0);
    let mut wanted = shown.to_vec();
    wanted.sort_unstable();
    world.keep_hot(&wanted);
    for &superchunk in shown {
        entity_rules::sheep::flock(&mut world.entities, superchunk, flock, &mut Rng::for_stream(!SEED, superchunk.0));
    }
    world.entities.apply();
    world
}

/// Cells of grass over every hot superchunk of `world`.
fn grass(world: &World) -> u64 {
    world.arena.superchunk_indices().into_iter().map(|superchunk| world.arena.superchunk_count(GRASS, superchunk) as u64).sum()
}

/// The superchunks of `world` that `ask` asks for, copied: each one's
/// grass, words as they are, and its sheep's cells.
fn copy(world: &World, ask: Ask) -> Vec<Cells> {
    let mut copied = Vec::new();
    for (x, y) in ask.asked() {
        let superchunk = SuperchunkIndex::from_cartesian(x, y);
        // Hot if its entities are held; cold, there are no cells to copy.
        let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
        let hot = world.entities.superchunk(superchunk).is_some();
        let (grass, sheep) = if hot { (layer(world, GRASS, superchunk), sheep(world, superchunk)) } else { (Vec::new(), Vec::new()) };
        copied.push(Cells { at: (x, y), hot, top_left: (left, top), grass, sheep });
    }
    copied
}

/// `superchunk`'s cells of `layer_type`: its chunks' words, one chunk
/// after another.
fn layer(world: &World, layer_type: LayerType, superchunk: SuperchunkIndex) -> Vec<u64> {
    let mut words = Vec::with_capacity(CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS);
    for chunk in superchunk.chunks() {
        match world.arena.bucket(BucketKey { layer_type, chunk }) {
            Some(bucket) => words.extend_from_slice(bucket.cells()),
            None => words.resize(words.len() + CHUNK_WORDS, 0),
        }
    }
    words
}

/// The cells `superchunk`'s sheep stand on, from its top left.
fn sheep(world: &World, superchunk: SuperchunkIndex) -> Vec<(u16, u16)> {
    let Some(kept) = world.entities.superchunk(superchunk) else {
        return Vec::new();
    };
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let mut cells = Vec::with_capacity(kept.len());
    for entity in kept.iter() {
        let at = entity.header.at.cartesian();
        cells.push(((at.x - left) as u16, (at.y - top) as u16));
    }
    cells
}
