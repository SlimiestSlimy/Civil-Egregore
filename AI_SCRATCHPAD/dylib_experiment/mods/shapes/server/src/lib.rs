//! The shapes mod's server bundle: a rule saying the common bundle's shape.

use engine_server::engine_common::{Registry, WORLD_NAME};
use engine_server::{ServerHandle, TICKS_RUN};
use shapes_common::{Shape, TIMES_REGISTERED};
use std::sync::atomic::Ordering;

#[unsafe(no_mangle)]
pub fn bundle_register(_registry: &mut Registry) {}

#[unsafe(no_mangle)]
pub fn bundle_start(registry: &Registry, handle: &mut ServerHandle) -> Result<(), String> {
    let hexagon = registry.library::<Shape>("shapes::hexagon")?;
    let world = WORLD_NAME.get().cloned().ok_or("the world's name unset: a copy of the engine of its own")?;
    let registered = TIMES_REGISTERED.load(Ordering::Relaxed);
    handle.add_rule(move |tick| {
        tick.said.push(format!("tick {} (the engine's count {}) of {world}: a {} of {} sides, its bundle registered {registered} time(s)", tick.tick, TICKS_RUN.load(Ordering::Relaxed), hexagon.name, hexagon.sides));
    });
    Ok(())
}
