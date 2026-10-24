//! The base game's windows, each with its code ([`menus`],
//! [`worldgen_sliders`]). The world's window opens with each world: a
//! button opening the world generation's window, and one resetting the
//! world. The world generation's window: a slider a worldgen parameter,
//! each its own gui type, setting what the world is reset with.

pub mod menus;
pub mod worldgen_sliders;

use crate::default_types::registered;
use crate::worldgen::calls::WorldgenCalls;
use crate::worldgen::WorldGeneration;
use protocol::type_registry::{GuiType, Hook, RegisteredType, Registration, TypeRegistry};
use std::sync::Arc;

/// The world's window: what opens with each world.
pub const WORLD_WINDOW: &str = "world";
/// The world generation's window: its sliders.
pub const WORLDGEN_WINDOW: &str = "world generation sliders";

/// The base game's windows and their guis registered in `type_registry`,
/// calling `worldgen_calls`: each one's code its library.
pub fn register(type_registry: &mut TypeRegistry, worldgen_calls: WorldgenCalls) -> Result<(), String> {
    let worldgen_window = gui_type(&registered(type_registry, Registration::gui(WORLDGEN_WINDOW, None).with_library(Arc::new(menus::window(worldgen_sliders::WINDOW_BLOCKS))))?);
    registered(type_registry, Registration::gui(WORLD_WINDOW, None).with_hooks(&[Hook::ShownByDefault]).with_library(Arc::new(menus::window(menus::WORLD_WINDOW_BLOCKS))))?;
    registered(type_registry, Registration::gui("world generation button", Some(WORLD_WINDOW)).with_library(Arc::new(menus::opening_button("world generation", worldgen_window))))?;
    registered(type_registry, Registration::gui("reset world button", Some(WORLD_WINDOW)).with_library(Arc::new(menus::calling_button("reset world", worldgen_calls.reset_world))))?;
    for (place, (parameter_name, _)) in WorldGeneration::DEFAULT.worldgen_parameters().into_iter().enumerate() {
        let slider = Registration::gui(&format!("{parameter_name} slider"), Some(WORLDGEN_WINDOW)).with_library(Arc::new(worldgen_sliders::slider(&parameter_name, place as u64, worldgen_calls)));
        registered(type_registry, slider)?;
    }
    Ok(())
}

/// The gui type `registered` is.
fn gui_type(registered: &RegisteredType) -> GuiType {
    match registered {
        RegisteredType::Gui { gui, .. } => *gui,
        _ => unreachable!("registered as a gui"),
    }
}
