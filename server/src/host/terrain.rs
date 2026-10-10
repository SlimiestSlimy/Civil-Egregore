//! What a client asks of a world's terrain where no frame brings it:
//! the heights about a superchunk, and the whole of the ground for a
//! map -- asked of the host and answered on a thread of its own, the
//! ticks never waiting, from how the world run is generated
//! (`docs/server.md`, "Terrain asked of the host").

pub use chunk_storage::Height;

use coordinates::SUPERCHUNK_SIDE_CELLS;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use worldgen::mesh::Lands;
use worldgen::Generation;

/// The heights a world is drawn between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levels {
    /// The lowest ground: the ocean's floor.
    pub ground: Height,
    /// The ocean's level.
    pub ocean: Height,
    /// The highest land.
    pub highest: Height,
}

impl Levels {
    /// Those of a world generated as `generation` says.
    pub(crate) fn of(generation: &Generation) -> Self {
        Self { ground: generation.shape.ground, ocean: generation.shape.ocean, highest: generation.shape.highest }
    }
}

/// What a cell is generated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// The ocean over it: nothing grows.
    Ocean,
    /// A tree.
    Tree,
    /// Grass, and no tree.
    Grass,
    /// Neither: dirt.
    Dirt,
}

/// The heights of a rectangle of cells, asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeightsAsk {
    /// The world they are of ([`super::frame::Frame::world`]).
    pub world: u64,
    /// The top left cell, `(x, y)` in the world: the rectangle goes
    /// round the world's edges.
    pub first: (u32, u32),
    /// Cells across and down.
    pub size: (u32, u32),
    /// The top left cells of the superchunks not to work out: those a
    /// frame brought the client. Their cells are answered as 0.
    pub skipped: Vec<(u32, u32)>,
}

/// A map asked for: a pixel the cell in its middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapAsk {
    /// The world it is of ([`super::frame::Frame::world`]).
    pub world: u64,
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size: (u32, u32),
    /// Whether the mesh's lines are asked for.
    pub borders: bool,
}

/// A pixel of a map: the cell in its middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapCell {
    /// Its height.
    pub height: Height,
    /// The height of the pixel above it: its own, where that is past
    /// the world's edge.
    pub above: Height,
    /// What it is generated with.
    pub cover: Cover,
    /// Whether it is no farther from a line of the mesh the land is
    /// made of than a pixel is across: false unless asked for.
    pub on_a_mesh_line: bool,
}

/// A map answered.
pub struct MapAnswer {
    /// What was asked.
    pub ask: MapAsk,
    /// The heights the world is drawn between.
    pub levels: Levels,
    /// Its pixels' cells, row by row: none past the world's edges.
    pub cells: Vec<Option<MapCell>>,
}

/// What the terrain's thread is sent.
pub(crate) enum TerrainRequest {
    /// The world run from now, by the host's own thread: its number,
    /// its seed and how it is generated.
    World(u64, u64, Box<Generation>),
    /// Heights asked for, and where to answer.
    Heights(HeightsAsk, Sender<Option<Vec<Height>>>),
    /// A map asked for, and where to answer.
    Map(MapAsk, Sender<Option<MapAnswer>>),
}

/// Where a client asks of the terrain of the world the host runs: each
/// call waits for its answer, on the caller's thread -- a thread other
/// than the window's own asks what takes long.
#[derive(Clone)]
pub struct TerrainAsker {
    /// Where the asks go.
    pub(crate) requests: Sender<TerrainRequest>,
}

impl TerrainAsker {
    /// The heights asked for, row by row: none if the host is gone or
    /// runs another world than the one asked of.
    pub fn heights(&self, ask: HeightsAsk) -> Option<Vec<Height>> {
        let (answer, answered) = channel();
        self.requests.send(TerrainRequest::Heights(ask, answer)).ok()?;
        answered.recv().ok().flatten()
    }

    /// The map asked for: none if the host is gone or runs another
    /// world than the one asked of.
    pub fn map(&self, ask: MapAsk) -> Option<MapAnswer> {
        let (answer, answered) = channel();
        self.requests.send(TerrainRequest::Map(ask, answer)).ok()?;
        answered.recv().ok().flatten()
    }
}

/// Starts the terrain's thread: where it is asked. It stops once every
/// asker and the host are gone.
pub(crate) fn start() -> Sender<TerrainRequest> {
    let (requests, asked) = channel();
    thread::Builder::new().name("terrain".to_string()).spawn(move || run(&asked)).expect("a thread for the terrain");
    requests
}

/// Answers what is asked of the world last told of, until no one is
/// left to ask.
fn run(asked: &Receiver<TerrainRequest>) {
    let mut of: Option<(u64, u64, Generation)> = None;
    while let Ok(request) = asked.recv() {
        let running = |world: u64| of.as_ref().filter(|(running, ..)| *running == world).map(|(_, seed, generation)| (*seed, generation));
        // An asker gone before its answer is nothing to tell anyone.
        match request {
            TerrainRequest::World(world, seed, generation) => of = Some((world, seed, *generation)),
            TerrainRequest::Heights(ask, answer) => _ = answer.send(running(ask.world).map(|(seed, generation)| heights(seed, generation, &ask))),
            TerrainRequest::Map(ask, answer) => _ = answer.send(running(ask.world).map(|(seed, generation)| map(seed, generation, ask))),
        }
    }
}

/// Rows of `rows` given out among the machine's threads, each thread
/// handed its first row's number and its rows, `width` items a row:
/// a cell is the same whoever works it out.
fn shared_out<T: Send>(items: &mut [T], width: usize, work: impl Fn(usize, &mut [T]) + Sync) {
    let threads = thread::available_parallelism().map_or(1, |threads| threads.get());
    let rows_each = (items.len() / width.max(1)).div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (part, rows) in items.chunks_mut(rows_each * width.max(1)).enumerate() {
            let work = &work;
            scope.spawn(move || work(part * rows_each, rows));
        }
    });
}

/// The heights `ask` asks for, of the world of `seed` generated as
/// `generation` says.
fn heights(seed: u64, generation: &Generation, ask: &HeightsAsk) -> Vec<Height> {
    let (width, side) = (ask.size.0 as usize, SUPERCHUNK_SIDE_CELLS);
    let mut heights = vec![0; width * ask.size.1 as usize];
    shared_out(&mut heights, width, |first_row, rows| {
        let mut lands = Lands::new(&generation.shape, seed);
        for (row, heights) in rows.chunks_mut(width).enumerate() {
            let (y, mut across) = (ask.first.1.wrapping_add((first_row + row) as u32), 0);
            while across < width {
                // The row's run within one superchunk.
                let x = ask.first.0.wrapping_add(across as u32);
                let run = ((side - x % side) as usize).min(width - across);
                if !ask.skipped.contains(&(x - x % side, y - y % side)) {
                    heights[across..across + run].iter_mut().zip(0u32..).for_each(|(height, along)| *height = lands.height(x.wrapping_add(along), y));
                }
                across += run;
            }
        }
    });
    heights
}

/// The map `ask` asks for, of the world of `seed` generated as
/// `generation` says.
fn map(seed: u64, generation: &Generation, ask: MapAsk) -> MapAnswer {
    let (width, levels) = (ask.size.0 as usize, Levels::of(generation));
    // The cell in the middle of a pixel, or none past the world's edges.
    let cell = |x: i64, y: i64| {
        let cell = |first: i64, pixel: i64| u32::try_from(first + pixel * i64::from(ask.step) + i64::from(ask.step) / 2).ok();
        Some((cell(ask.first.0, x)?, cell(ask.first.1, y)?))
    };
    let growth = &generation.growth(seed);
    let mut cells = vec![None; width * ask.size.1 as usize];
    shared_out(&mut cells, width, |first_row, rows| {
        // Each thread its own: both keep what they found about the last cell.
        let (mut lands, mut growth) = (Lands::new(&generation.shape, seed), *growth);
        for (row, cells) in rows.chunks_mut(width).enumerate() {
            let y = (first_row + row) as i64;
            for (x, pixel) in cells.iter_mut().enumerate() {
                let Some((cell_x, cell_y)) = cell(x as i64, y) else {
                    continue;
                };
                let height = lands.height(cell_x, cell_y);
                let above = cell(x as i64, y - 1).map_or(height, |(x, y)| lands.height(x, y));
                let cover = if height < levels.ocean {
                    Cover::Ocean
                } else {
                    let grown = growth.at(cell_x, cell_y);
                    match (grown.tree, grown.grass) {
                        (Some(_), _) => Cover::Tree,
                        (None, true) => Cover::Grass,
                        (None, false) => Cover::Dirt,
                    }
                };
                let on_a_mesh_line = ask.borders && lands.line(cell_x, cell_y).1 < u64::from(ask.step);
                *pixel = Some(MapCell { height, above, cover, on_a_mesh_line });
            }
        }
    });
    MapAnswer { ask, levels, cells }
}
