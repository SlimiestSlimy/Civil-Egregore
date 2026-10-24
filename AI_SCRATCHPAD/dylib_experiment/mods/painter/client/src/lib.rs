//! The painter mod's client bundle: another mod's library cast down, and
//! another mod's component queried.

use engine_client::bevy::prelude::*;
use engine_client::engine_common::Registry;
use engine_client::{FrameNumber, Said};
use shapes_client::{DrawnShape, Palette};

#[unsafe(no_mangle)]
pub fn bundle_register(_registry: &mut Registry) {}

#[unsafe(no_mangle)]
pub fn bundle_start(registry: &Registry, app: &mut App) -> Result<(), String> {
    let palette = registry.library::<Palette>("shapes drawn::palette")?;
    let wrong_cast_refused = registry.library::<Palette>("shapes::hexagon").is_err();
    app.add_systems(Update, move |shapes: Query<&DrawnShape>, frame: Res<FrameNumber>, mut said: ResMut<Said>| {
        for shape in &shapes {
            let colour = palette.colour_of(frame.0);
            said.0.push(format!("frame {}: painter sees shapes' drawn shape of {} sides, paints it {colour} (a wrong cast refused: {wrong_cast_refused})", frame.0, shape.sides));
        }
    });
    Ok(())
}
