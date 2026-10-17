//! What a client asks of a world's terrain where no frame brings it --
//! the heights about a superchunk, the whole of the ground for a map --
//! and what it is answered with.

pub use chunk_storage::Height;

use utilities::wire::{WireReader, Wired, WiredStruct};
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
    pub fn from_generation(world_generation: &WorldGeneration) -> Self {
        let land_shape = &world_generation.land_shape;
        let nearly_flat = land_shape.highest_land <= land_shape.ocean + worldgen::STEP;
        Self { lowest_height: land_shape.ocean_floor, ocean: land_shape.ocean, highest_land: if nearly_flat { land_shape.ocean + worldgen::LandShape::DEFAULT.highest_land } else { land_shape.highest_land } }
    }
}

/// Whoever is asked of a world's terrain: each ask waits for its
/// answer, on the asker's thread -- a thread other than the window's
/// own asks what takes long. Asked of any thread.
pub trait AsksTerrain: Send + Sync {
    /// The heights asked for, row by row: none if the server is gone or
    /// runs another world than the one asked of.
    fn heights(&self, heights_ask: HeightsAsk) -> Option<Vec<Height>>;

    /// The map asked for: none if the server is gone or runs another
    /// world than the one asked of.
    fn map(&self, map_ask: MapAsk) -> Option<MapAnswer>;
}

/// What a cell is generated with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellCover {
    /// The ocean over it: nothing grows.
    Ocean,
    /// A tree.
    Tree,
    /// Grass, and no tree.
    Grass,
    /// Neither: dirt.
    #[default]
    Dirt,
}

/// The heights of a rectangle of cells, asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeightsAsk {
    /// The world they are of ([`super::frame::ViewportFrame::world`]).
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapAsk {
    /// The world it is of ([`super::frame::ViewportFrame::world`]).
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Default)]
pub struct MapAnswer {
    /// What was asked.
    pub map_ask: MapAsk,
    /// The heights the world is drawn between.
    pub drawn_heights: DrawnHeightRange,
    /// Its pixels' cells, row by row: none past the world's edges.
    pub cells: Vec<Option<MapCell>>,
}

/// On the wire, its place in the enum's order.
impl Wired for CellCover {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(*self as u64);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = [Self::Ocean, Self::Tree, Self::Grass, Self::Dirt].get(usize::try_from(wire.next_word()?).ok()?).copied()?;
        Some(())
    }
}

impl WiredStruct for DrawnHeightRange {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|range| &mut range.lowest_height, |range| &mut range.ocean, |range| &mut range.highest_land];
}

impl WiredStruct for HeightsAsk {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|ask| &mut ask.world, |ask| &mut ask.first_cell_cartesian, |ask| &mut ask.size_in_cells, |ask| &mut ask.skipped_corners_cartesian];
}

impl WiredStruct for MapAsk {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|ask| &mut ask.world, |ask| &mut ask.first_cell_cartesian, |ask| &mut ask.step, |ask| &mut ask.size_in_pixels, |ask| &mut ask.mesh_lines_asked];
}

impl WiredStruct for MapCell {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|cell| &mut cell.height, |cell| &mut cell.height_above, |cell| &mut cell.cell_cover, |cell| &mut cell.on_a_mesh_line];
}

impl WiredStruct for MapAnswer {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|answer| &mut answer.map_ask, |answer| &mut answer.drawn_heights, |answer| &mut answer.cells];
}
