//! Civil Egregore's menus, laid over whatever window shows the world,
//! a part a module:
//!
//! | module | what it is |
//! |---|---|
//! | [`main_menu`] | what the window opens on: a new world, a world saved, or leaving |
//! | [`options`] | what Escape opens over a world: going on, saving it, opening another, leaving |
//! | [`sliders`] | the numbers tuned by eye (`utilities::tuning`), a slider each, in groups |
//! | [`rows`] | what the main menu and the options are both made of: rows a click each |
//!
//! It knows nothing of the world or of what draws it. What it is told
//! comes with [`Gui`] and [`options::Options::name`]; what it has to
//! say is read off it: which screen is up ([`Screen`]), what of the
//! pointer, the wheel and the keys its menus took ([`Captured`]), the
//! numbers its sliders set ([`CurrentTuning`]), and a message for each
//! world to be made ([`Make`]), opened ([`Open`]) or saved ([`Save`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod main_menu;
pub mod options;
pub mod rows;
pub mod sliders;

use bevy::prelude::*;
use utilities::tuning::Tuning;

/// The menus, added to a window's app: their parts made at startup,
/// and worked every frame ([`Worked`]).
pub struct Gui {
    /// Names the worlds there are to open, asked each time a menu
    /// lists them.
    pub worlds: fn() -> Vec<String>,
}

/// The menus' work of a frame: what is to see what they did the same
/// frame runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Worked;

/// What the window shows: the main menu, until a world is made or
/// opened from it; then the world.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// The main menu, and nothing behind it.
    #[default]
    MainMenu,
    /// A world.
    World,
}

/// A new world to be made: from `seed`, or one drawn at random if none
/// is given, the rest as `tuning` has it -- the sliders of the groups
/// that make worlds, when it was set up.
#[derive(Message)]
pub struct Make {
    /// The seed typed, if one was.
    pub seed: Option<u64>,
    /// The numbers tuned then.
    pub tuning: Tuning,
}

/// The numbers tuned by eye, as the sliders have set them: the
/// menus' to set, a world made from them, the near view shaded by
/// them. As the machine last kept them to begin with.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CurrentTuning(pub Tuning);

/// The world of this name in the worlds' folder to be opened.
#[derive(Message)]
pub struct Open(pub String);

/// The world run to be saved under this name in the worlds' folder.
#[derive(Message)]
pub struct Save(pub String);

/// What of the pointer, the wheel and the keys the menus took this
/// frame: the world behind them is not steered by it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Captured {
    /// The pointer's buttons.
    pub pointer: bool,
    /// The wheel.
    pub wheel: bool,
    /// The keys.
    pub keys: bool,
}

/// Offers the sliders as the screen has it: every group while a new
/// world is set up, all but those only of setting one up over a world,
/// none over the rest of the main menu or under the options.
fn offer(mut sliders: ResMut<sliders::Sliders>, screen: Res<Screen>, menu: Res<main_menu::MainMenu>, options: Res<options::Options>) {
    sliders.offer(match *screen {
        Screen::MainMenu if menu.setting_up() => sliders::Offered::Everything,
        Screen::World if !options.open() => sliders::Offered::Running,
        _ => sliders::Offered::Hidden,
    });
}

/// What the menus took, worked out last of their work.
fn capture(mut captured: ResMut<Captured>, screen: Res<Screen>, options: Res<options::Options>, sliders: Res<sliders::Sliders>, window: Single<&Window>) {
    let menu = *screen == Screen::MainMenu || options.open();
    *captured = Captured { pointer: menu || sliders.held(), wheel: menu || sliders.over(&window), keys: menu || sliders.typing() };
}

impl Plugin for Gui {
    /// Its parts and its work -- and the numbers tuned, as the machine
    /// last kept them.
    fn build(&self, app: &mut App) {
        app.insert_resource(CurrentTuning(utilities::tuning::kept()))
            .init_resource::<Screen>()
            .init_resource::<Captured>()
            .init_resource::<sliders::Sliders>()
            .insert_resource(options::Options::listing(self.worlds))
            .insert_resource(main_menu::MainMenu::listing(self.worlds))
            .add_message::<Make>()
            .add_message::<Open>()
            .add_message::<Save>()
            .add_systems(Startup, (sliders::spawn, options::spawn, main_menu::spawn))
            .add_systems(Update, (main_menu::work, options::work, offer, sliders::toggle, sliders::scroll, sliders::slide, sliders::tell, capture).chain().in_set(Worked));
    }
}
