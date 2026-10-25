//! The server's handles, as a type's code is given them: as its world
//! starts ([`ServerSetUp`]), and between two ticks, the world whole and
//! consistent ([`BetweenTicks`]).

use crate::type_registry::FrozenTypeRegistry;
use std::path::Path;

/// What the server hands a type's code as its world starts: the world's
/// frozen types, and the code's mod's folder of the world's mod config.
pub struct ServerSetUp<'setup> {
    /// The world's types, frozen.
    frozen_type_registry: &'setup FrozenTypeRegistry,
    /// The code's mod's folder of the world's mod config.
    mod_config_folder: &'setup Path,
}

impl<'setup> ServerSetUp<'setup> {
    /// The handle to a world of `frozen_type_registry`, for code of the mod
    /// whose config is in `mod_config_folder`.
    pub fn new(frozen_type_registry: &'setup FrozenTypeRegistry, mod_config_folder: &'setup Path) -> Self {
        Self { frozen_type_registry, mod_config_folder }
    }

    /// The world's types, frozen: what the code finds the types it works
    /// on in.
    pub fn frozen_type_registry(&self) -> &FrozenTypeRegistry {
        self.frozen_type_registry
    }

    /// The code's mod's folder of the world's mod config: its files the
    /// mod's own, read and written as it will -- the server reads none.
    pub fn mod_config_folder(&self) -> &Path {
        self.mod_config_folder
    }
}

/// The server between two ticks, as a type's code run then reaches it:
/// the world whole and consistent.
pub trait BetweenTicks {
    /// Resets the world once the code is done: made again from its start
    /// in its folder, of the same types and seed, generated as its config
    /// then says -- everything else of it gone.
    fn reset_world(&mut self);
}
