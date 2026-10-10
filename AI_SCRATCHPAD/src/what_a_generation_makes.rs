//! What a generation makes of a world's first superchunks: the cells
//! of each layer, counted over a small world forced hot.

use server::{Size, Start};
use utilities::commands::Given;
use worldgen::{Generation, ONE};
use type_registry::{GRASS, WALL_EAST, WALL_SOUTH, WET};

/// The share of the plain's cells with grass, in thousandths.
pub const GRASS_THOUSANDTHS: &str = "grass thousandths";

/// Superchunks along the world's side.
pub const SIDE: &str = "side";

/// Runs the probe on a plain ([`Generation::plain`]): a line a layer.
pub fn run(given: &Given) -> Result<(), String> {
    let (thousandths, side): (u64, u32) = (given.number(GRASS_THOUSANDTHS)?, given.number(SIDE)?);
    let world = server::start(Start { generation: Generation::plain(thousandths * ONE / 1000), size: Size::Limited { side, forced: true }, sheep: 0, ..Start::default() });
    println!("layer,cells");
    for (name, layer_type) in [("grass", GRASS), ("wet", WET), ("wall east", WALL_EAST), ("wall south", WALL_SOUTH)] {
        let cells: u64 = world.arena.superchunk_indices().into_iter().map(|superchunk| world.arena.superchunk_count(layer_type, superchunk) as u64).sum();
        println!("{name},{cells}");
    }
    println!("superchunks,{}", world.arena.superchunk_indices().len());
    Ok(())
}
