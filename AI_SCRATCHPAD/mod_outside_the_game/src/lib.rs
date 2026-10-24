//! A client bundle built apart against the shipped libraries: a component
//! of its own (Bevy's derive), the shapes mod's component queried, its
//! palette cast down and called.

use engine_client::bevy::prelude::*;
use engine_client::engine_common::Registry;
use engine_client::{FrameNumber, Said};
use shapes_client::{DrawnShape, Palette};

/// A component of this bundle's own.
#[derive(Component)]
struct SeenApart(u32);

#[unsafe(no_mangle)]
pub fn bundle_register(_registry: &mut Registry) {}

#[unsafe(no_mangle)]
pub fn bundle_start(registry: &Registry, app: &mut App) -> Result<(), String> {
    let palette = registry.library::<Palette>("shapes drawn::palette")?;
    app.add_systems(Update, move |mut commands: Commands, shapes: Query<(Entity, &DrawnShape), Without<SeenApart>>, seen: Query<&SeenApart>, frame: Res<FrameNumber>, mut said: ResMut<Said>| {
        for (entity, shape) in &shapes {
            commands.entity(entity).insert(SeenApart(shape.sides));
        }
        for seen_apart in &seen {
            said.0.push(format!("frame {}: apart, kit and own bitflags's own component holds {} sides, {}", frame.0, seen_apart.0, palette.colour_of(frame.0)));
        }
    });
    Ok(())
}

bitflags::bitflags! {
    /// A dependency of the mod's own, shared with Bevy, its feature its own.
    pub struct OwnFlags: u8 {
        const SEEN = 1;
    }
}
