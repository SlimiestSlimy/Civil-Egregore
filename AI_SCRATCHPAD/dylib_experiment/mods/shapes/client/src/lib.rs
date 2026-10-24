//! The shapes mod's client bundle: the common bundle's shape spawned as a
//! Bevy component of the mod's own, a palette for other mods, and work
//! on the engine's task pool.

use engine_client::bevy::prelude::*;
use engine_client::bevy::tasks::AsyncComputeTaskPool;
use engine_client::engine_common::{Registry, WORLD_NAME};
use engine_client::{FrameNumber, Said};
use shapes_common::{Shape, TIMES_REGISTERED};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// A shape drawn: a component of this mod's, queried by another's.
#[derive(Component)]
pub struct DrawnShape {
    pub sides: u32,
}

/// Colours, a library of this bundle's for other mods' client bundles.
pub struct Palette {
    pub colours: Vec<&'static str>,
}

/// Palettes made: a static of this bundle's, read by another mod's.
pub static PALETTES_MADE: AtomicU32 = AtomicU32::new(0);

impl Palette {
    /// Code of this bundle's, called by another mod's: never inlined, so
    /// the caller links this library.
    #[inline(never)]
    pub fn colour_of(&self, frame: u64) -> String {
        format!("{} (palette {} of {})", self.colours[frame as usize % self.colours.len()], frame % 3, PALETTES_MADE.load(Ordering::Relaxed))
    }
}

#[unsafe(no_mangle)]
pub fn bundle_register(registry: &mut Registry) {
    registry.register("shapes drawn::palette", Arc::new(Palette { colours: vec!["red", "green", "blue"] }));
    PALETTES_MADE.fetch_add(1, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub fn bundle_start(registry: &Registry, app: &mut App) -> Result<(), String> {
    let hexagon = registry.library::<Shape>("shapes::hexagon")?;
    let world = WORLD_NAME.get().cloned().ok_or("the world's name unset: a copy of the engine of its own")?;
    let registered = TIMES_REGISTERED.load(Ordering::Relaxed);
    let sides = hexagon.sides;
    app.add_systems(Startup, move |mut commands: Commands, mut said: ResMut<Said>| {
        commands.spawn(DrawnShape { sides });
        said.0.push(format!("shapes drawn: spawned a {} in {world}, its common bundle registered {registered} time(s)", hexagon.name));
    });
    app.add_systems(Update, |frame: Res<FrameNumber>, mut said: ResMut<Said>| {
        let frame_number = frame.0;
        let products = AsyncComputeTaskPool::get().scope(|scope| {
            for part in 1..=4u64 {
                scope.spawn(async move { (part * frame_number, std::thread::current().name().unwrap_or("?").to_string()) });
            }
        });
        said.0.push(format!("frame {frame_number}: shapes drawn's tasks on the engine's pool gave {products:?}"));
    });
    Ok(())
}
