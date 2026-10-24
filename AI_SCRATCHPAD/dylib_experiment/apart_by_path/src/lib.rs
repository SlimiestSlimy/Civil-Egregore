//! A server bundle built apart, by path.

use engine_server::engine_common::{Registry, WORLD_NAME};
use engine_server::ServerHandle;
use shapes_common::Shape;

#[unsafe(no_mangle)]
pub fn bundle_register(_registry: &mut Registry) {}

#[unsafe(no_mangle)]
pub fn bundle_start(registry: &Registry, handle: &mut ServerHandle) -> Result<(), String> {
    let hexagon = registry.library::<Shape>("shapes::hexagon")?;
    let world = WORLD_NAME.get().cloned().ok_or("the world's name unset")?;
    let me = env!("CARGO_PKG_NAME");
    handle.add_rule(move |tick| tick.said.push(format!("tick {}: {me} sees a {} in {world}", tick.tick, hexagon.name)));
    Ok(())
}
