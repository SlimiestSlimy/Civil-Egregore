//! The options, over a world: opened and closed by Escape, a row each
//! -- going on, saving the world, opening one, leaving
//! (`docs/gui.md`, "Options").

use crate::rows::{self, Bar, WorldListing, RowLook, MenuRow, RowText, MenuShade};
use crate::sliders::Sliders;
use crate::{OpenWorld, Save, ShownScreen};
use bevy::app::AppExit;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// The most letters a world's name has.
const MOST_NAME_LETTERS: usize = 32;

/// What a click on a row of the options does.
#[derive(Clone, Copy)]
enum OptionsClick {
    /// Nothing: a row that only says something.
    DoesNothing,
    /// Closes the options.
    GoesOn,
    /// Saves the world: under its name, or one typed first.
    Saves,
    /// Saves the world under the name typed.
    SavesNamed,
    /// Lists the worlds there are to open.
    ListsWorlds,
    /// Goes back to the options' first page.
    GoesBack,
    /// Opens a world, by its place among those listed.
    OpensWorld(usize),
    /// Leaves Civil Egregore.
    LeavesGame,
}

/// What of the options is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionsPage {
    /// Their first page.
    FirstPage,
    /// The worlds there are to open.
    Worlds,
    /// A name for the world, typed.
    NamingWorld,
}

/// Marks the options' parts.
#[derive(Component, Clone)]
pub struct Part;

/// The options: whether they are open, and what of them is shown.
#[derive(Resource)]
pub struct Options {
    /// Whether they are open.
    options_open: bool,
    /// What of them is shown.
    shown_page: OptionsPage,
    /// The name of the world run, if it has one.
    world_name: Option<String>,
    /// The name being typed for it.
    typed_name: String,
    /// Whether the name typed was refused: a world there is has it.
    name_taken: bool,
    /// Names the worlds there are.
    world_lister: fn() -> Vec<String>,
    /// The worlds, as last listed.
    world_listing: WorldListing,
}

impl Options {
    /// The options, closed, their worlds named by `world_lister`.
    pub fn with_world_lister(world_lister: fn() -> Vec<String>) -> Self {
        Self { options_open: false, shown_page: OptionsPage::FirstPage, world_name: None, typed_name: String::new(), name_taken: false, world_lister, world_listing: WorldListing::default() }
    }

    /// Whether they are open.
    pub fn is_open(&self) -> bool {
        self.options_open
    }

    /// The name of the world run, as they were told it.
    pub fn current_world_name(&self) -> Option<&str> {
        self.world_name.as_deref()
    }

    /// Tells them the name of the world run, if it has one: what it
    /// is saved under.
    pub fn set_world_name(&mut self, world_name: Option<String>) {
        self.world_name = world_name;
    }

    /// Says the world is to be saved under the name typed, and closes
    /// the options -- unless none is typed, or a world there is has it.
    fn save_named(&mut self, save: &mut MessageWriter<Save>) {
        let Some(valid_name) = utilities::settings::world_name(&self.typed_name) else {
            return;
        };
        self.name_taken = (self.world_lister)().contains(&valid_name);
        if !self.name_taken {
            save.write(Save(valid_name));
            self.options_open = false;
        }
    }

    /// The rows shown, from the top.
    fn shown_rows(&self) -> Vec<MenuRow<OptionsClick>> {
        match self.shown_page {
            OptionsPage::FirstPage => {
                let save = self.world_name.as_ref().map_or("save the world".to_string(), |world_name| format!("save {world_name}"));
                vec![MenuRow::new("Civil Egregore", RowLook::OnlySays, OptionsClick::DoesNothing), MenuRow::new("go on (Escape)", RowLook::ActsOnClick, OptionsClick::GoesOn), MenuRow::new(save, RowLook::ActsOnClick, OptionsClick::Saves), MenuRow::new("open a world", RowLook::ActsOnClick, OptionsClick::ListsWorlds), MenuRow::new("exit", RowLook::ActsOnClick, OptionsClick::LeavesGame)]
            }
            OptionsPage::NamingWorld => {
                let name_taken_row = self.name_taken.then(|| MenuRow::new("there is a world of that name", RowLook::OnlySays, OptionsClick::DoesNothing));
                [MenuRow::new("< options  |  a name for the world", RowLook::GoesBack, OptionsClick::GoesBack), MenuRow::new(format!("{}_", self.typed_name), RowLook::OnlySays, OptionsClick::DoesNothing), MenuRow::new("save (Enter)", RowLook::ActsOnClick, OptionsClick::SavesNamed)].into_iter().chain(name_taken_row).collect()
            }
            OptionsPage::Worlds => self.world_listing.world_rows(OptionsClick::GoesBack, OptionsClick::OpensWorld),
        }
    }
}

/// The options' parts, hidden.
pub fn spawn_options(mut bevy_commands: Commands) {
    rows::spawn_menu_parts(&mut bevy_commands, Part);
}

/// Opens and closes the options by Escape over a world -- unless a
/// slider's value is being typed -- does what a row clicked does,
/// takes the keys typing a name, goes through the worlds by the wheel,
/// and shows the rows as they are.
#[allow(clippy::too_many_arguments)]
pub fn update_options(
    mut options: ResMut<Options>,
    shown_screen: Res<ShownScreen>,
    sliders: Res<Sliders>,
    pressed_keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    mut shade_visibility: Single<&mut Visibility, (With<MenuShade>, With<Part>)>,
    mut bars: Query<(&Bar, &mut Node, &mut BackgroundColor), With<Part>>,
    mut words: Query<(&RowText, &mut Text), With<Part>>,
    mut message_writers: (MessageWriter<AppExit>, MessageWriter<OpenWorld>, MessageWriter<Save>),
    mut typed_keys: MessageReader<KeyboardInput>,
) {
    let (exit_writer, open_world_writer, save) = &mut message_writers;
    if *shown_screen != ShownScreen::World {
        options.options_open = false;
    } else if pressed_keys.just_pressed(KeyCode::Escape) && !sliders.typing_value() {
        // Opened on their first page.
        (options.options_open, options.shown_page) = (!options.options_open, OptionsPage::FirstPage);
    } else if options.options_open {
        match rows::clicked_row_does(&options.shown_rows(), &window, &mouse_buttons) {
            Some(OptionsClick::GoesOn) => options.options_open = false,
            Some(OptionsClick::Saves) => match options.world_name.clone() {
                Some(world_name) => {
                    save.write(Save(world_name));
                    options.options_open = false;
                }
                None => (options.shown_page, options.typed_name, options.name_taken) = (OptionsPage::NamingWorld, String::new(), false),
            },
            Some(OptionsClick::SavesNamed) => options.save_named(save),
            Some(OptionsClick::ListsWorlds) => (options.world_listing, options.shown_page) = (WorldListing::from_lister(options.world_lister), OptionsPage::Worlds),
            Some(OptionsClick::GoesBack) => options.shown_page = OptionsPage::FirstPage,
            Some(OptionsClick::OpensWorld(place)) => {
                open_world_writer.write(OpenWorld(options.world_listing.worlds[place].clone()));
                options.options_open = false;
            }
            Some(OptionsClick::LeavesGame) => _ = exit_writer.write(AppExit::Success),
            Some(OptionsClick::DoesNothing) | None => {}
        }
    }
    if options.options_open && options.shown_page == OptionsPage::NamingWorld {
        let mut name_line = std::mem::take(&mut options.typed_name);
        let enter_pressed = rows::type_into(&mut name_line, &mut typed_keys, |typed_letter| typed_letter.is_alphanumeric() || "-_. ".contains(typed_letter), MOST_NAME_LETTERS);
        options.typed_name = name_line;
        if enter_pressed {
            options.save_named(save);
        }
    } else {
        // Keys not typed into a name are no one's: none is kept for later.
        typed_keys.clear();
    }
    if options.options_open && options.shown_page == OptionsPage::Worlds {
        options.world_listing.scroll_by_wheel(wheel.delta.y);
    }
    rows::show_rows(options.options_open, &options.shown_rows(), &mut shade_visibility, &mut bars, &mut words);
}
