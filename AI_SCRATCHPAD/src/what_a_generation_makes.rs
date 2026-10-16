//! What a generation makes of a world's first superchunks: the cells
//! of each layer, counted over a small world forced hot.

use server::{WorldSize, WorldStart};
use utilities::commands::GivenCommand;
use worldgen::{WorldGeneration, FRACTION_ONE};
use type_registry::{GRASS, WALL_EAST, WALL_SOUTH, WET};

/// The share of the plain's cells with grass, in thousandths.
pub const GRASS_THOUSANDTHS: &str = "grass thousandths";

/// Superchunks along the world's side.
pub const WORLD_SIDE_SUPERCHUNKS: &str = "side";

/// Runs the probe on a plain ([`Generation::plain`]): a line a layer.
pub fn run_probe(given_parameters: &GivenCommand) -> Result<(), String> {
    let (grass_thousandths, world_side_superchunks): (u64, u32) = (given_parameters.argument_number(GRASS_THOUSANDTHS)?, given_parameters.argument_number(WORLD_SIDE_SUPERCHUNKS)?);
    let world = server::start_world(WorldStart { world_generation: WorldGeneration::plain(grass_thousandths * FRACTION_ONE / 1000), world_size: WorldSize::SquareOfSide { side_superchunks: world_side_superchunks, forced_hot: true }, sheep_per_flock: 0, ..WorldStart::default() });
    println!("layer,cells");
    for (layer_name, layer_type) in [("grass", GRASS), ("wet", WET), ("wall east", WALL_EAST), ("wall south", WALL_SOUTH)] {
        let cells: u64 = world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(layer_type, superchunk) as u64).sum();
        println!("{layer_name},{cells}");
    }
    println!("superchunks,{}", world.bitmap_arena().superchunk_indices().len());
    Ok(())
}
