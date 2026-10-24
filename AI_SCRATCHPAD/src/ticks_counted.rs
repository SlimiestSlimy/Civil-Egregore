//! A fixed amount of ticking, for counting instructions (callgrind)
//! rather than timing: a world of a side's superchunks every one hot,
//! sheep in each, ticked a given number of times on one seed; then its
//! hash, the same of any build of the same simulation. Run at N ticks
//! and at 2N, the difference is N ticks' cost alone, without the world's
//! making.

use crate::ProbeParameters;
use protocol::server_parameters::{CleanServerParameters, ParameterValue, SEED, SIDE, WORLDGEN};
use protocol::type_registry::TypeRegistry;
use std::sync::Arc;

/// Superchunks along the world's side.
pub const WORLD_SIDE: &str = "world_side";

/// Ticks run.
pub const TICKS_RUN: &str = "ticks";

/// Sheep each superchunk is generated with.
pub const SHEEP_EACH_SUPERCHUNK: &str = "sheep_each_superchunk";

/// Runs the probe: one line, the ticks run and the world's hash after them.
pub fn run_probe(given_parameters: &ProbeParameters) -> Result<(), String> {
    let (world_side, ticks_run, sheep_each_superchunk): (u64, u64, u64) = (given_parameters.argument_number(WORLD_SIDE)?, given_parameters.argument_number(TICKS_RUN)?, given_parameters.argument_number(SHEEP_EACH_SUPERCHUNK)?);
    let seed = u64::from_str_radix(given_parameters.argument_text(crate::SEED)?.trim_start_matches("0x"), 16).map_err(|_| "--seed: a hexadecimal number".to_string())?;
    let worldgen_parameters = basegame::worldgen::WorldGeneration { sheep_per_superchunk: sheep_each_superchunk, ..basegame::worldgen::WorldGeneration::DEFAULT }.worldgen_parameters();
    let worldgen = worldgen_parameters.into_iter().fold(CleanServerParameters::default(), |worldgen, (parameter_name, value)| worldgen.with(parameter_name, ParameterValue::Hex(value)));
    let parameters = CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(world_side as i64)).with(WORLDGEN, ParameterValue::Nested(worldgen));
    let mut type_registry = TypeRegistry::for_new_world();
    basegame::default_types::register_types(&mut type_registry)?;
    let frozen_type_registry = Arc::new(type_registry.freeze().map_err(|why_not| format!("{why_not:?}"))?);
    let mut world = server::start_world(&parameters, &frozen_type_registry).map_err(|why_not| why_not.to_string())?;
    for _ in 0..ticks_run {
        world.tick();
    }
    let world_hash = server::world_hash(&mut world);
    println!("ticks {ticks_run}, at tick {}, cells hash {:#x}, entities hash {:#x}", world_hash.tick, world_hash.cells, world_hash.entities);
    Ok(())
}
