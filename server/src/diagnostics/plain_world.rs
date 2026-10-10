//! The world the diagnostics tick: a plain (`Generation::plain`) with a
//! side, forced hot, made as every world is ([`crate::start`]) -- so a
//! rule is measured alone, the terrain taking no part.

use crate::{Size, Start, World};
use coordinates::square_side;
use worldgen::Generation;
#[cfg(doc)]
use worldgen::ONE;

/// A plain forced hot of at least `superchunks` superchunks -- a square
/// of them, so more where they make none -- with grass on `grass_cover`
/// of [`ONE`] of its cells and `sheep` sheep on each superchunk, ticked on
/// `threads` threads.
pub fn plain_world(superchunks: u32, grass_cover: u64, sheep: usize, threads: usize) -> World {
    let generation = Generation::plain(grass_cover);
    crate::start(Start { generation, size: Size::Limited { side: square_side(superchunks), forced: true }, threads: Some(threads), sheep, ..Start::default() })
}

/// Superchunks `world` holds hot.
pub fn superchunks(world: &World) -> usize {
    world.arena.superchunk_indices().len()
}
