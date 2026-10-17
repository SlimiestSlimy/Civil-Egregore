//! The world the in-bin tests tick: a plain (`Generation::plain`) with a
//! side, forced hot, made as every world is ([`crate::start_world`]) -- so a
//! rule is measured alone, the terrain taking no part.

use crate::{WorldSize, WorldStart, World};
use coordinates::square_side;
use worldgen::WorldGeneration;

/// A plain forced hot of at least `superchunks` superchunks -- a square
/// of them, so more where they make none -- with grass on `grass_cover`
/// of [`worldgen::FRACTION_ONE`] of its cells and `sheep_per_flock` sheep on each superchunk, ticked on
/// `thread_count` threads.
pub fn plain_world(superchunks: u32, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> World {
    crate::start_world(plain_start(superchunks, grass_cover, sheep_per_flock, thread_count))
}

/// How [`plain_world`] is started: what whoever shows the same world
/// in a window hands the host.
pub fn plain_start(superchunks: u32, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> WorldStart {
    WorldStart { world_generation: WorldGeneration::plain(grass_cover), world_size: WorldSize::SquareOfSide { side_superchunks: square_side(superchunks), forced_hot: true }, thread_count: Some(thread_count), sheep_per_flock, ..WorldStart::default() }
}

/// Superchunks `world` holds hot.
pub fn superchunks(world: &World) -> usize {
    world.bitmap_arena.superchunk_indices().len()
}

/// How the plain `server pasture` ticks is started, with grass on
/// `grass_thousandths` of its cells and `sheep_per_flock` on each of its
/// `superchunks`, on every thread: for the same world shown in a
/// window.
pub fn pasture_start(superchunks: u32, grass_thousandths: u64, sheep_per_flock: usize) -> WorldStart {
    WorldStart { thread_count: None, ..plain_start(superchunks, grass_thousandths * worldgen::FRACTION_ONE / 1000, sheep_per_flock, 0) }
}
