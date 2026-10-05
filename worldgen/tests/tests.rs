//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use coordinates::place_from_cartesian;
use worldgen::Terrain;

/// The height of the cell `(x, y)` of a superchunk's `terrain`, from
/// its top left.
pub fn at(terrain: &Terrain, x: u32, y: u32) -> u16 {
    terrain.height(place_from_cartesian(x, y))
}

/// Whether `terrain` keeps a wall the `way`-th way at the cell `(x, y)`.
pub fn walled(terrain: &Terrain, way: usize, x: u32, y: u32) -> bool {
    terrain.walled(way, place_from_cartesian(x, y))
}
