//! A mock world with sheep on it: the instructions' mock world
//! (`instructions::mock_world`), a flock on each of its superchunks.

use crate::sheep::flock;
use instructions::mock_world::MockWorld;
use instructions::Rng;

/// A mock world of `count` superchunks, each with grass drawn on
/// `grass_cells` cells and `sheep` sheep on cells drawn at random.
pub fn mock_world_with_sheep(count: u32, grass_cells: usize, sheep: usize) -> MockWorld {
    let mut world = MockWorld::grass_on_dirt(count, grass_cells);
    let mut random = Rng::new(0x5EE9);
    for superchunk in world.superchunks().to_vec() {
        flock(&mut world.between_ticks(), superchunk, sheep, &mut random);
    }
    world.settle();
    world
}
