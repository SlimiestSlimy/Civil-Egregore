//! What the window opens on, before any world: a new world set up, a
//! world saved opened, or leaving -- a row each
//! (`docs/gui.md`, "The main menu").

use crate::gui::rows::{self, Bar, WorldListing, RowLook, MenuRow, RowText, MenuShade};
use crate::gui::sliders::Sliders;
use crate::gui::{CurrentTuning, MakeWorld, OpenWorld, ShownScreen};
use bevy::app::AppExit;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// What a click on a row of the main menu does.
#[derive(Clone, Copy)]
enum MainMenuClick {
    /// Nothing: a row that only says something.
    DoesNothing,
    /// Sets up a new world.
    SetsUp,
    /// Makes the world set up.
    MakesWorld,
    /// Lists the worlds there are to open.
    ListsWorlds,
    /// Goes back to the first page.
    GoesBack,
    /// Opens a world, by its place among those listed.
    OpensWorld(usize),
    /// Leaves Civil Egregore.
    LeavesGame,
}

/// What of the main menu is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MainMenuPage {
    /// Its first page.
    FirstPage,
    /// A new world being set up.
    NewWorld,
    /// The worlds there are to open.
    Worlds,
}

/// Marks the main menu's parts.
#[derive(Component, Clone)]
pub struct Part;

/// The main menu: what of it is shown, and what is typed into it.
#[derive(Resource)]
pub struct MainMenu {
    /// What of it is shown.
    shown_page: MainMenuPage,
    /// The new world's seed, typed: hexadecimal digits, or none.
    seed: String,
    /// Names the worlds there are.
    world_lister: fn() -> Vec<String>,
    /// The worlds, as last listed.
    world_listing: WorldListing,
}

impl MainMenu {
    /// The main menu, on its first page, its worlds named by `world_lister`.
    pub fn with_world_lister(world_lister: fn() -> Vec<String>) -> Self {
        Self { shown_page: MainMenuPage::FirstPage, seed: String::new(), world_lister, world_listing: WorldListing::default() }
    }

    /// Whether a new world is being set up: every slider is offered.
    pub fn setting_up(&self) -> bool {
        self.shown_page == MainMenuPage::NewWorld
    }

    /// The seed typed, if one is.
    fn seed(&self) -> Option<u64> {
        utilities::seed::of_hex(&self.seed)
    }

    /// The rows shown, from the top.
    fn shown_rows(&self) -> Vec<MenuRow<MainMenuClick>> {
        match self.shown_page {
            MainMenuPage::FirstPage => vec![MenuRow::new("Civil Egregore", RowLook::OnlySays, MainMenuClick::DoesNothing), MenuRow::new("new world", RowLook::ActsOnClick, MainMenuClick::SetsUp), MenuRow::new("open a world", RowLook::ActsOnClick, MainMenuClick::ListsWorlds), MenuRow::new("exit", RowLook::ActsOnClick, MainMenuClick::LeavesGame)],
            MainMenuPage::NewWorld => vec![
                MenuRow::new("< back  |  a new world", RowLook::GoesBack, MainMenuClick::GoesBack),
                MenuRow::new(format!("seed 0x{}_", self.seed), RowLook::OnlySays, MainMenuClick::DoesNothing),
                MenuRow::new(if self.seed.is_empty() { "none typed: one drawn at random" } else { "in hexadecimal" }, RowLook::OnlySays, MainMenuClick::DoesNothing),
                MenuRow::new("U: its size, its camera, its sheep, its land", RowLook::OnlySays, MainMenuClick::DoesNothing),
                MenuRow::new("make the world (Enter)", RowLook::ActsOnClick, MainMenuClick::MakesWorld),
            ],
            MainMenuPage::Worlds => self.world_listing.world_rows(MainMenuClick::GoesBack, MainMenuClick::OpensWorld),
        }
    }
}

/// The main menu's parts, hidden until shown.
pub fn spawn_main_menu(mut bevy_commands: Commands) {
    rows::spawn_menu_parts(&mut bevy_commands, Part);
}

/// Does what a row clicked does -- unless the pointer is the sliders'
/// -- takes the keys typing a seed, goes through the worlds by the
/// wheel, goes back a page by Escape, and shows the rows as they are:
/// all while the window shows it.
#[allow(clippy::too_many_arguments)]
pub fn update_main_menu(
    mut main_menu_state: ResMut<MainMenu>,
    mut shown_screen: ResMut<ShownScreen>,
    sliders: Res<Sliders>,
    tuning: Res<CurrentTuning>,
    pressed_keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    mut shade_visibility: Single<&mut Visibility, (With<MenuShade>, With<Part>)>,
    mut bars: Query<(&Bar, &mut Node, &mut BackgroundColor), With<Part>>,
    mut words: Query<(&RowText, &mut Text), With<Part>>,
    mut message_writers: (MessageWriter<AppExit>, MessageWriter<MakeWorld>, MessageWriter<OpenWorld>),
    mut typed_keys: MessageReader<KeyboardInput>,
) {
    let (exit_writer, make_world_writer, open_world_writer) = &mut message_writers;
    let menu_shown = *shown_screen == ShownScreen::MainMenu;
    let mut makes_world = false;
    if menu_shown && !sliders.typing_value() && pressed_keys.just_pressed(KeyCode::Escape) {
        main_menu_state.shown_page = MainMenuPage::FirstPage;
    }
    if menu_shown && !sliders.holds_left_button() && !sliders.pointer_over(&window) {
        match rows::clicked_row_does(&main_menu_state.shown_rows(), &window, &mouse_buttons) {
            Some(MainMenuClick::SetsUp) => main_menu_state.shown_page = MainMenuPage::NewWorld,
            Some(MainMenuClick::MakesWorld) => makes_world = true,
            Some(MainMenuClick::ListsWorlds) => (main_menu_state.world_listing, main_menu_state.shown_page) = (WorldListing::from_lister(main_menu_state.world_lister), MainMenuPage::Worlds),
            Some(MainMenuClick::GoesBack) => main_menu_state.shown_page = MainMenuPage::FirstPage,
            Some(MainMenuClick::OpensWorld(place)) => {
                open_world_writer.write(OpenWorld(main_menu_state.world_listing.worlds[place].clone()));
                *shown_screen = ShownScreen::World;
            }
            Some(MainMenuClick::LeavesGame) => _ = exit_writer.write(AppExit::Success),
            Some(MainMenuClick::DoesNothing) | None => {}
        }
    }
    if menu_shown && main_menu_state.shown_page == MainMenuPage::NewWorld && !sliders.typing_value() {
        let mut seed = std::mem::take(&mut main_menu_state.seed);
        makes_world |= rows::type_into(&mut seed, &mut typed_keys, |typed_character| typed_character.is_ascii_hexdigit(), 16);
        main_menu_state.seed = seed;
    } else {
        // Keys not typed into a seed are no one's: none is kept for later.
        typed_keys.clear();
    }
    if makes_world {
        make_world_writer.write(MakeWorld { seed: main_menu_state.seed(), tuning: tuning.0 });
        *shown_screen = ShownScreen::World;
    }
    if menu_shown && main_menu_state.shown_page == MainMenuPage::Worlds && !sliders.pointer_over(&window) {
        main_menu_state.world_listing.scroll_by_wheel(wheel.delta.y);
    }
    let menu_still_shown = *shown_screen == ShownScreen::MainMenu;
    rows::show_rows(menu_still_shown, &main_menu_state.shown_rows(), &mut shade_visibility, &mut bars, &mut words);
}
