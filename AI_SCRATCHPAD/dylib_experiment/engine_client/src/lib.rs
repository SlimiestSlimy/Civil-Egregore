//! The client's public API, as client_api: Bevy re-exported whole (one
//! copy of it, inside this library), and what a client bundle's start
//! gets -- the frozen registry and the Bevy App.

pub use bevy;
pub use engine_common;
use bevy::prelude::*;

/// What the window says, lines pushed by any bundle's systems.
#[derive(Resource, Default)]
pub struct Said(pub Vec<String>);

/// A resource of the engine's, set by the binary, read by mods.
#[derive(Resource)]
pub struct FrameNumber(pub u64);

/// A client bundle's start.
pub type ClientStart = fn(&engine_common::Registry, &mut App) -> Result<(), String>;
