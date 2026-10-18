//! The areas the tiers' tests make terrain of: in a folder of its own,
//! so no test program of its own, a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use server::simulation::simulation_api::{CellArea, CellCartesian, RandomSource, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE_CELL};

/// The area a superchunk's side across whose top left is `top_left`.
pub fn area_from(top_left: CellCartesian) -> CellArea {
    CellArea { top_left, side_cells: SUPERCHUNK_SIDE_CELLS }
}

/// The area in the world's middle.
pub fn middle_area() -> CellArea {
    area_from(WORLD_MIDDLE_CELL)
}

/// The area as wide as `area` `(dx, dy)` areas from it.
pub fn area_beside(area: CellArea, dx: i32, dy: i32) -> CellArea {
    let side = area.side_cells as i32;
    CellArea { top_left: CellCartesian { x: area.top_left.x.wrapping_add_signed(dx * side), y: area.top_left.y.wrapping_add_signed(dy * side) }, ..area }
}

/// An area anywhere in the world with another east and south of it,
/// drawn by `seeded_rng`.
pub fn area_drawn(seeded_rng: &mut RandomSource) -> CellArea {
    let last_drawable = (1u64 << u32::BITS) / u64::from(SUPERCHUNK_SIDE_CELLS) - 1;
    area_from(CellCartesian { x: seeded_rng.draw_below(last_drawable) as u32 * SUPERCHUNK_SIDE_CELLS, y: seeded_rng.draw_below(last_drawable) as u32 * SUPERCHUNK_SIDE_CELLS })
}
