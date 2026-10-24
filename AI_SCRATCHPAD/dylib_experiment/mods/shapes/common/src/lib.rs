//! The shapes mod's common bundle: what both of its sides know.

use engine_common::Registry;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// A shape, the mod's own type, cast down on both sides.
pub struct Shape {
    pub name: &'static str,
    pub sides: u32,
}

/// Times this bundle registered: a static of the mod's, read by its
/// other bundles -- one copy, or they read 0.
pub static TIMES_REGISTERED: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub fn bundle_register(registry: &mut Registry) {
    registry.register("shapes::hexagon", Arc::new(Shape { name: "hexagon", sides: 6 }));
    TIMES_REGISTERED.fetch_add(1, Ordering::Relaxed);
}
