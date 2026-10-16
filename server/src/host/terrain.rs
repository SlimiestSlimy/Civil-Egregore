//! What a client asks of a world's terrain where no frame brings it:
//! the heights about a superchunk, and the whole of the ground for a
//! map -- asked of the host and answered on a thread of its own, the
//! ticks never waiting, from how the world run is generated
//! (`docs/server.md`, "Terrain asked of the host").

pub use chunk_storage::Height;

use coordinates::SUPERCHUNK_SIDE_CELLS;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use worldgen::mesh::WorldLand;
use worldgen::WorldGeneration;

/// The heights a world is drawn between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawnHeightRange {
    /// The lowest ground: the ocean's floor.
    pub lowest_height: Height,
    /// The ocean's level.
    pub ocean: Height,
    /// The highest land.
    pub highest_land: Height,
}

impl DrawnHeightRange {
    /// Those of a world generated as `world_generation` says. A world with
    /// no relief to speak of -- a plain, a step above its ocean -- is
    /// drawn between the levels worlds have unless told otherwise:
    /// its land is low land, not a peak.
    pub(crate) fn from_generation(world_generation: &WorldGeneration) -> Self {
        let land_shape = &world_generation.land_shape;
        let nearly_flat = land_shape.highest_land <= land_shape.ocean + worldgen::STEP;
        Self { lowest_height: land_shape.ocean_floor, ocean: land_shape.ocean, highest_land: if nearly_flat { land_shape.ocean + worldgen::LandShape::DEFAULT.highest_land } else { land_shape.highest_land } }
    }
}

/// What a cell is generated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellCover {
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
    pub first_cell_cartesian: (u32, u32),
    /// Cells across and down.
    pub size_in_cells: (u32, u32),
    /// The top left cells of the superchunks not to work out: those a
    /// frame brought the client. Their cells are answered as 0.
    pub skipped_corners_cartesian: Vec<(u32, u32)>,
}

/// A map asked for: a pixel the cell in its middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapAsk {
    /// The world it is of ([`super::frame::Frame::world`]).
    pub world: u64,
    /// The cell at its top left pixel's top left, `(x, y)` in the
    /// world: past the world's edges it may be.
    pub first_cell_cartesian: (i64, i64),
    /// Cells along a pixel's side.
    pub step: u32,
    /// Pixels across and down.
    pub size_in_pixels: (u32, u32),
    /// Whether the mesh's lines are asked for.
    pub mesh_lines_asked: bool,
}

/// A pixel of a map: the cell in its middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapCell {
    /// Its height.
    pub height: Height,
    /// The height of the pixel above it: its own, where that is past
    /// the world's edge.
    pub height_above: Height,
    /// What it is generated with.
    pub cell_cover: CellCover,
    /// Whether it is no farther from a line of the mesh the land is
    /// made of than a pixel is across: false unless asked for.
    pub on_a_mesh_line: bool,
}

/// A map answered.
pub struct MapAnswer {
    /// What was asked.
    pub map_ask: MapAsk,
    /// The heights the world is drawn between.
    pub drawn_heights: DrawnHeightRange,
    /// Its pixels' cells, row by row: none past the world's edges.
    pub cells: Vec<Option<MapCell>>,
}

/// What the terrain's thread is sent.
pub(crate) enum TerrainRequest {
    /// The world run from now, by the host's own thread: its number,
    /// its seed and how it is generated.
    World(u64, u64, Box<WorldGeneration>),
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
    pub(crate) request_sender: Sender<TerrainRequest>,
}

impl TerrainAsker {
    /// The heights asked for, row by row: none if the host is gone or
    /// runs another world than the one asked of.
    pub fn heights(&self, heights_ask: HeightsAsk) -> Option<Vec<Height>> {
        let (answer_sender, answer_receiver) = channel();
        self.request_sender.send(TerrainRequest::Heights(heights_ask, answer_sender)).ok()?;
        answer_receiver.recv().ok().flatten()
    }

    /// The map asked for: none if the host is gone or runs another
    /// world than the one asked of.
    pub fn map(&self, map_ask: MapAsk) -> Option<MapAnswer> {
        let (answer_sender, answer_receiver) = channel();
        self.request_sender.send(TerrainRequest::Map(map_ask, answer_sender)).ok()?;
        answer_receiver.recv().ok().flatten()
    }
}

/// Starts the terrain's thread: where it is asked. It stops once every
/// asker and the host are gone.
pub(crate) fn start_terrain_thread() -> Sender<TerrainRequest> {
    let (request_sender, request_receiver) = channel();
    thread::Builder::new().name("terrain".to_string()).spawn(move || answer_terrain_requests(&request_receiver)).expect("a thread for the terrain");
    request_sender
}

/// Answers what is asked of the world last told of, until no one is
/// left to ask.
fn answer_terrain_requests(request_receiver: &Receiver<TerrainRequest>) {
    let mut world_told: Option<(u64, u64, WorldGeneration)> = None;
    while let Ok(terrain_request) = request_receiver.recv() {
        let seed_if_running = |world: u64| world_told.as_ref().filter(|(told_world_number, ..)| *told_world_number == world).map(|(_, seed, told_generation)| (*seed, told_generation));
        // An asker gone before its answer is nothing to tell anyone.
        match terrain_request {
            TerrainRequest::World(world, seed, boxed_generation) => world_told = Some((world, seed, *boxed_generation)),
            TerrainRequest::Heights(heights_ask, heights_answer) => _ = heights_answer.send(seed_if_running(heights_ask.world).map(|(seed, heights_generation)| heights(seed, heights_generation, &heights_ask))),
            TerrainRequest::Map(map_ask, map_answer) => _ = map_answer.send(seed_if_running(map_ask.world).map(|(seed, map_generation)| map(seed, map_generation, map_ask))),
        }
    }
}

/// Rows of `part_rows` given out among the machine's threads, each thread
/// handed its first row's number and its rows, `width` items a row:
/// a cell is the same whoever works it out.
fn shared_out<T: Send>(row_items: &mut [T], width: usize, row_work: impl Fn(usize, &mut [T]) + Sync) {
    let thread_count = thread::available_parallelism().map_or(1, |available_threads| available_threads.get());
    let rows_each = (row_items.len() / width.max(1)).div_ceil(thread_count).max(1);
    thread::scope(|thread_scope| {
        for (part, part_rows) in row_items.chunks_mut(rows_each * width.max(1)).enumerate() {
            let shared_row_work = &row_work;
            thread_scope.spawn(move || shared_row_work(part * rows_each, part_rows));
        }
    });
}

/// The heights `heights_ask` asks for, of the world of `seed` generated as
/// `world_generation` says.
fn heights(seed: u64, world_generation: &WorldGeneration, heights_ask: &HeightsAsk) -> Vec<Height> {
    let (width, superchunk_side_cells) = (heights_ask.size_in_cells.0 as usize, SUPERCHUNK_SIDE_CELLS);
    let mut heights = vec![0; width * heights_ask.size_in_cells.1 as usize];
    shared_out(&mut heights, width, |first_row, part_rows| {
        let mut mesh_lands = WorldLand::new(&world_generation.land_shape, seed);
        for (row_in_part, heights) in part_rows.chunks_mut(width).enumerate() {
            let (y, mut cells_across) = (heights_ask.first_cell_cartesian.1.wrapping_add((first_row + row_in_part) as u32), 0);
            while cells_across < width {
                // The row's run within one superchunk.
                let x = heights_ask.first_cell_cartesian.0.wrapping_add(cells_across as u32);
                let span_in_superchunk = ((superchunk_side_cells - x % superchunk_side_cells) as usize).min(width - cells_across);
                if !heights_ask.skipped_corners_cartesian.contains(&(x - x % superchunk_side_cells, y - y % superchunk_side_cells)) {
                    heights[cells_across..cells_across + span_in_superchunk].iter_mut().zip(0u32..).for_each(|(height, cells_along)| *height = mesh_lands.height(x.wrapping_add(cells_along), y));
                }
                cells_across += span_in_superchunk;
            }
        }
    });
    heights
}

/// The map `map_ask` asks for, of the world of `seed` generated as
/// `world_generation` says.
fn map(seed: u64, world_generation: &WorldGeneration, map_ask: MapAsk) -> MapAnswer {
    let (width, drawn_heights) = (map_ask.size_in_pixels.0 as usize, DrawnHeightRange::from_generation(world_generation));
    // The cell in the middle of a pixel, or none past the world's edges.
    let cell = |x: i64, y: i64| {
        let cell = |first_coordinate: i64, pixel_offset: i64| u32::try_from(first_coordinate + pixel_offset * i64::from(map_ask.step) + i64::from(map_ask.step) / 2).ok();
        Some((cell(map_ask.first_cell_cartesian.0, x)?, cell(map_ask.first_cell_cartesian.1, y)?))
    };
    let world_growth = &world_generation.world_growth(seed);
    let mut cells = vec![None; width * map_ask.size_in_pixels.1 as usize];
    shared_out(&mut cells, width, |first_row, part_rows| {
        // Each thread its own: both keep what they found about the last cell.
        let (mut mesh_lands, mut thread_growth) = (WorldLand::new(&world_generation.land_shape, seed), *world_growth);
        for (row_in_part, cells) in part_rows.chunks_mut(width).enumerate() {
            let y = (first_row + row_in_part) as i64;
            for (x, map_pixel) in cells.iter_mut().enumerate() {
                let Some((cell_x, cell_y)) = cell(x as i64, y) else {
                    continue;
                };
                let height = mesh_lands.height(cell_x, cell_y);
                let height_above = cell(x as i64, y - 1).map_or(height, |(x, y)| mesh_lands.height(x, y));
                let cell_cover = if height < drawn_heights.ocean {
                    CellCover::Ocean
                } else {
                    let grown_here = thread_growth.grown_at(cell_x, cell_y);
                    match (grown_here.tree, grown_here.has_grass) {
                        (Some(_), _) => CellCover::Tree,
                        (None, true) => CellCover::Grass,
                        (None, false) => CellCover::Dirt,
                    }
                };
                let on_a_mesh_line = map_ask.mesh_lines_asked && mesh_lands.inland_and_line_distance(cell_x, cell_y).1 < u64::from(map_ask.step);
                *map_pixel = Some(MapCell { height, height_above, cell_cover, on_a_mesh_line });
            }
        }
    });
    MapAnswer { map_ask, drawn_heights, cells }
}
