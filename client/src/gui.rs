//! Civil Egregore's menus, laid over whatever window shows the world:
//! the main menu, the options, the sliders, and the rows the first two
//! are made of. It knows nothing of the world or of what draws it.
//!
//! The design: `docs/gui.md`; function by function:
//! `docs/reference.md`.

pub mod main_menu;
pub mod options;
pub mod rows;
pub mod sliders;

use bevy::prelude::*;
use utilities::tuning::Tuning;

/// The menus, added to a window's app: their parts made at startup,
/// and worked every frame ([`MenusWorked`]).
pub struct GuiPlugin {
    /// Names the worlds there are to open, asked each time a menu
    /// lists them.
    pub worlds: fn() -> Vec<String>,
}

/// The menus' work of a frame: what is to see what they did the same
/// frame runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenusWorked;

/// What the window shows: the main menu, until a world is made or
/// opened from it; then the world.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShownScreen {
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
pub struct MakeWorld {
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
pub struct OpenWorld(pub String);

/// The world run to be saved under this name in the worlds' folder.
#[derive(Message)]
pub struct Save(pub String);

/// What of the pointer, the wheel and the keys the menus took this
/// frame: the world behind them is not steered by it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CapturedInput {
    /// The pointer's buttons.
    pub pointer_buttons: bool,
    /// The wheel.
    pub wheel: bool,
    /// The keys.
    pub keyboard_keys: bool,
}

/// Offers the sliders as the screen has it: every group while a new
/// world is set up, all but those only of setting one up over a world,
/// none over the rest of the main menu or under the options.
fn offer_sliders(mut sliders: ResMut<sliders::Sliders>, shown_screen: Res<ShownScreen>, main_menu_state: Res<main_menu::MainMenu>, options: Res<options::Options>) {
    sliders.offer_groups(match *shown_screen {
        ShownScreen::MainMenu if main_menu_state.setting_up() => sliders::SlidersOffered::EveryGroup,
        ShownScreen::World if !options.is_open() => sliders::SlidersOffered::RunningGroups,
        _ => sliders::SlidersOffered::NoneOffered,
    });
}

/// What the menus took, worked out last of their work.
fn capture_input(mut captured_input: ResMut<CapturedInput>, shown_screen: Res<ShownScreen>, options: Res<options::Options>, sliders: Res<sliders::Sliders>, window: Single<&Window>) {
    let menu_covers_window = *shown_screen == ShownScreen::MainMenu || options.is_open();
    *captured_input = CapturedInput { pointer_buttons: menu_covers_window || sliders.holds_left_button(), wheel: menu_covers_window || sliders.pointer_over(&window), keyboard_keys: menu_covers_window || sliders.typing_value() };
}

impl Plugin for GuiPlugin {
    /// Its parts and its work -- and the numbers tuned, as the machine
    /// last kept them.
    fn build(&self, window_app: &mut App) {
        window_app.insert_resource(CurrentTuning(utilities::tuning::load_from_settings()))
            .init_resource::<ShownScreen>()
            .init_resource::<CapturedInput>()
            .init_resource::<sliders::Sliders>()
            .insert_resource(options::Options::with_world_lister(self.worlds))
            .insert_resource(main_menu::MainMenu::with_world_lister(self.worlds))
            .add_message::<MakeWorld>()
            .add_message::<OpenWorld>()
            .add_message::<Save>()
            .add_systems(Startup, (sliders::spawn_sliders, options::spawn_options, main_menu::spawn_main_menu))
            .add_systems(Update, (main_menu::update_main_menu, options::update_options, offer_sliders, sliders::toggle, sliders::scroll_sliders, sliders::work_slider_clicks, sliders::tell_what_slider_does, capture_input).chain().in_set(MenusWorked));
    }
}
