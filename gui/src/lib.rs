//! TileSim's menus, laid over whatever window shows the world: the
//! options Escape opens ([`options`]), the sliders ([`sliders`]) and
//! the numbers they tune ([`tuning`]).
//!
//! It knows nothing of the world or of what draws it. What it is told
//! comes with [`Gui`]; what it has to say is read off it: the numbers
//! ([`tuning::now`]), the seed drawn ([`tuning::seed_drawn`]), whether
//! the options are open ([`options::Options::open`]), and a message
//! for each world chosen to be opened ([`options::Chosen`]) and each
//! time the world is to be saved ([`options::Save`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod options;
pub mod sliders;
pub mod tuning;

use bevy::prelude::*;

/// The menus, added to a window's app: their parts made at startup,
/// and worked every frame ([`Worked`]).
pub struct Gui {
    /// Names the worlds there are to open, asked each time the options
    /// list them.
    pub worlds: fn() -> Vec<String>,
}

/// The menus' work of a frame: what is to see what they did the same
/// frame runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Worked;

impl Plugin for Gui {
    fn build(&self, app: &mut App) {
        app.init_resource::<sliders::Hands>()
            .insert_resource(options::Options::listing(self.worlds))
            .add_message::<options::Chosen>()
            .add_message::<options::Save>()
            .add_systems(Startup, (sliders::setup, options::setup))
            .add_systems(Update, (options::work, sliders::toggle, sliders::scroll, sliders::slide, sliders::tell).chain().in_set(Worked));
    }
}
