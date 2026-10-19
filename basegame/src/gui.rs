//! The base game's windows: registered whether or not the client is
//! built -- a world's types are the same headless or windowed -- their
//! code, Bevy's, only with the `client` feature ([`menus`],
//! [`worldgen_sliders`]). The world's window opens with each world: a
//! button opening the world generation's window, and one resetting the
//! world. The world generation's window: a slider a worldgen parameter,
//! each its own gui type, setting what the world is reset with.

#[cfg(feature = "client")]
pub mod menus;
#[cfg(feature = "client")]
pub mod worldgen_sliders;

use crate::default_types::registered;
use crate::worldgen::calls::WorldgenCalls;
use crate::worldgen::WorldGeneration;
use protocol::type_registry::{GuiType, Hook, RegisteredType, Registration, TypeRegistry};
#[cfg(feature = "client")]
use std::sync::Arc;

/// The world's window: what opens with each world.
pub const WORLD_WINDOW: &str = "world";
/// The world generation's window: its sliders.
pub const WORLDGEN_WINDOW: &str = "world generation sliders";

/// The base game's windows and their guis registered in `type_registry`,
/// calling `worldgen_calls`: each one's code its library, built with the
/// client.
#[cfg_attr(not(feature = "client"), allow(unused_variables))]
pub fn register(type_registry: &mut TypeRegistry, worldgen_calls: WorldgenCalls) -> Result<(), String> {
    let worldgen_window = Registration::gui(WORLDGEN_WINDOW, None);
    #[cfg(feature = "client")]
    let worldgen_window = worldgen_window.with_library(Arc::new(menus::window(worldgen_sliders::WINDOW_BLOCKS)));
    let worldgen_window = gui_type(&registered(type_registry, worldgen_window)?);
    let world_window = Registration::gui(WORLD_WINDOW, None).with_hooks(&[Hook::ShownByDefault]);
    #[cfg(feature = "client")]
    let world_window = world_window.with_library(Arc::new(menus::window(menus::WORLD_WINDOW_BLOCKS)));
    registered(type_registry, world_window)?;
    let open_worldgen = Registration::gui("world generation button", Some(WORLD_WINDOW));
    #[cfg(feature = "client")]
    let open_worldgen = open_worldgen.with_library(Arc::new(menus::opening_button("world generation", worldgen_window)));
    registered(type_registry, open_worldgen)?;
    let reset_world = Registration::gui("reset world button", Some(WORLD_WINDOW));
    #[cfg(feature = "client")]
    let reset_world = reset_world.with_library(Arc::new(menus::calling_button("reset world", worldgen_calls.reset_world)));
    registered(type_registry, reset_world)?;
    for (place, (parameter_name, _)) in WorldGeneration::DEFAULT.worldgen_parameters().into_iter().enumerate() {
        let slider = Registration::gui(&format!("{parameter_name} slider"), Some(WORLDGEN_WINDOW));
        #[cfg(feature = "client")]
        let slider = slider.with_library(Arc::new(worldgen_sliders::slider(&parameter_name, place as u64, worldgen_calls)));
        registered(type_registry, slider)?;
    }
    #[cfg(not(feature = "client"))]
    let _ = worldgen_window;
    Ok(())
}

/// The gui type `registered` is.
fn gui_type(registered: &RegisteredType) -> GuiType {
    match registered {
        RegisteredType::Gui { gui, .. } => *gui,
        _ => unreachable!("registered as a gui"),
    }
}
