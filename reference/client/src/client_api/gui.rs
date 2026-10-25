//! Gui types as the client runs them ([`GuiCode`]): each a
//! self-contained Bevy gui, built in its place in a window's grid of
//! blocks -- a gui type naming no parent is a window, one naming a
//! parent is built into that window's grid, after those registered
//! before it. The one window system frames each window, drags it, and
//! closes it; a window with the hook `ShownByDefault` opens with each
//! world, any other when a gui asks for it ([`OpenWindow`]).

use crate::client_api::ServerCalls;
use bevy::ecs::system::EntityCommands;
use bevy::prelude::Event;
use protocol::type_registry::GuiType;
use std::sync::Arc;

/// Pixels along a block's side: a window's grid is of blocks, every gui
/// type's place in it so many blocks across and down.
pub const BLOCK_PIXELS: f32 = 28.0;

/// What a gui type is built with: its own type, and where it calls the
/// functions the server's types let a client call.
#[derive(Clone)]
pub struct GuiContext {
    /// Its own type.
    pub gui: GuiType,
    /// Where it calls the server's functions.
    pub server_calls: ServerCalls,
}

/// A gui type built: its code given the entity of its place -- a node
/// filling it -- to build its Bevy gui into, scenes, observers and all.
pub type GuiBuild = dyn Fn(&mut EntityCommands, &GuiContext) + Send + Sync;

/// What builds a gui type: its library, the client's.
#[derive(Clone)]
pub struct GuiCode {
    /// Its place's size, in blocks across and down: a window's, its grid's.
    pub blocks: (u32, u32),
    /// What builds it.
    pub build: Arc<GuiBuild>,
}

impl GuiCode {
    /// A gui `blocks` across and down, built by `build`.
    pub fn new(blocks: (u32, u32), build: impl Fn(&mut EntityCommands, &GuiContext) + Send + Sync + 'static) -> Self {
        Self { blocks, build: Arc::new(build) }
    }
}

impl std::fmt::Debug for GuiCode {
    /// Its size: what it builds, only its code knows.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.debug_struct("GuiCode").field("blocks", &self.blocks).finish_non_exhaustive()
    }
}

/// The window of this type opened, if it is not open -- brought to the
/// front if it is: what a gui's code triggers to open another.
#[derive(Event, Clone, Copy, Debug)]
pub struct OpenWindow(pub GuiType);
