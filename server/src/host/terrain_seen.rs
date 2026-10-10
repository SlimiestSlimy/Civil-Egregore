//! What a client asks of a world's terrain where no frame brings it:
//! the ground past the superchunks hot, and the whole of it for a map
//! -- answered from how the world is generated, the same a superchunk
//! is made from. A client asks here and names nothing under the server.

pub use chunk_storage::Height;
pub use worldgen::Generation;

use coordinates::SuperchunkIndex;
use worldgen::mesh::Lands;
use worldgen::Growth;

/// The heights of a world, cell by cell: what one thread asks, the
/// lands about the last cell asked kept for the next.
pub struct HeightsSeen {
    /// The world's lands.
    lands: Lands,
}

impl HeightsSeen {
    /// Those of the world made from `seed` as `generation` says.
    pub fn of(generation: &Generation, seed: u64) -> Self {
        Self { lands: Lands::new(&generation.shape, seed) }
    }

    /// The height of the cell at `(x, y)`.
    pub fn height(&mut self, x: u32, y: u32) -> Height {
        self.lands.height(x, y)
    }

    /// How far the cell at `(x, y)` is from the nearest line of the
    /// mesh the land is made of, in cells.
    pub fn cells_from_a_mesh_line(&mut self, x: u32, y: u32) -> u64 {
        self.lands.line(x, y).1
    }
}

/// What a dry cell is generated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// A tree.
    Tree,
    /// Grass, and no tree.
    Grass,
    /// Neither: dirt.
    Dirt,
}

/// What grows where in a world as it is generated: shared by every
/// thread that asks.
pub struct CoverSeen {
    /// What grows where.
    growth: Growth,
}

impl CoverSeen {
    /// That of the world made from `seed` as `generation` says.
    pub fn of(generation: &Generation, seed: u64) -> Self {
        Self { growth: generation.growth(seed) }
    }

    /// What the cell at `(x, y)` is generated with, were it dry.
    pub fn cover(&self, x: u32, y: u32) -> Cover {
        let grown = self.growth.at(x, y);
        match (grown.tree, grown.grass) {
            (Some(_), _) => Cover::Tree,
            (None, true) => Cover::Grass,
            (None, false) => Cover::Dirt,
        }
    }
}

/// The heights a world generated as `generation` says is drawn
/// between: its lowest ground, its ocean's level, its highest land.
pub fn levels(generation: &Generation) -> Levels {
    Levels { ground: generation.shape.ground, ocean: generation.shape.ocean, highest: generation.shape.highest }
}

/// The heights a world is drawn between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    /// The lowest ground: the ocean's floor.
    pub ground: Height,
    /// The ocean's level.
    pub ocean: Height,
    /// The highest land.
    pub highest: Height,
}

/// Whether a wall stands between two cells beside one another, of
/// these heights.
pub fn walled(one: Height, other: Height) -> bool {
    worldgen::wall(one, other)
}

/// The height of the cell at `place` in its superchunk, from the height
/// words a frame brings ([`crate::host::frame::Cells::heights`]).
pub fn height_in_frame(height_words: &[u64], place: usize) -> Height {
    chunk_storage::height_in(height_words, place)
}

/// The first seed from `from` on whose world, generated as
/// `generation` says, has land about `near`.
pub fn seed_with_land(from: u64, generation: &Generation, near: SuperchunkIndex) -> u64 {
    worldgen::seed_with_land(from, &generation.shape, near)
}
