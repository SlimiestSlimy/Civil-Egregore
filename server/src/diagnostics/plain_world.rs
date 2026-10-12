//! The world the in-bin tests tick: a plain (`Generation::plain`) with a
//! side, forced hot, made as every world is ([`crate::start`]) -- so a
//! rule is measured alone, the terrain taking no part.

use crate::{Size, Start, World};
use coordinates::square_side;
use worldgen::Generation;

/// A plain forced hot of at least `superchunks` superchunks -- a square
/// of them, so more where they make none -- with grass on `grass_cover`
/// of [`worldgen::ONE`] of its cells and `sheep` sheep on each superchunk, ticked on
/// `threads` threads.
pub fn plain_world(superchunks: u32, grass_cover: u64, sheep: usize, threads: usize) -> World {
    crate::start(plain_start(superchunks, grass_cover, sheep, threads))
}

/// How [`plain_world`] is started: what whoever shows the same world
/// in a window hands the host.
pub fn plain_start(superchunks: u32, grass_cover: u64, sheep: usize, threads: usize) -> Start {
    Start { generation: Generation::plain(grass_cover), size: Size::Limited { side: square_side(superchunks), forced: true }, threads: Some(threads), sheep, ..Start::default() }
}

/// Superchunks `world` holds hot.
pub fn superchunks(world: &World) -> usize {
    world.arena.superchunk_indices().len()
}

/// How the plain `server pasture` ticks is started, with grass on
/// `grass_thousandths` of its cells and `sheep` on each of its
/// `superchunks`, on every thread: for the same world shown in a
/// window.
pub fn pasture_start(superchunks: u32, grass_thousandths: u64, sheep: usize) -> Start {
    Start { threads: None, ..plain_start(superchunks, grass_thousandths * worldgen::ONE / 1000, sheep, 0) }
}
