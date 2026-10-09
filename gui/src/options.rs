//! The options, over a world: opened and closed by Escape, a row each
//! ([`crate::rows`]) -- going on, saving the world, opening one, and
//! leaving Civil Egregore.
//!
//! Saving says the world is to be saved ([`Save`]) under the name the
//! window gave for it ([`Options::name`]); a world with none yet is
//! named first, the name typed -- what a folder may be named
//! (`utilities::settings::world_name`), the name being its folder's
//! -- and not one a world there is has.
//!
//! Opening a world lists those there are ([`crate::Gui::worlds`]), a
//! row each, the wheel going through more than fit: a click on one
//! says it is to be opened ([`Open`]) and closes the options.
//!
//! While they are open the pointer, the wheel and the keys are theirs
//! ([`crate::Captured`]); the world ticks on behind them. Leaving is no
//! more than closing the window does: it is here to be found.

use crate::rows::{self, Bar, Listing, Look, Row, Says, Shade};
use crate::sliders::Sliders;
use crate::{Open, Save, Screen};
use bevy::app::AppExit;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// The most letters a world's name has.
const NAME: usize = 32;

/// What a click on a row of the options does.
#[derive(Clone, Copy)]
enum Does {
    /// Nothing: a row that only says something.
    Nothing,
    /// Closes the options.
    GoesOn,
    /// Saves the world: under its name, or one typed first.
    Saves,
    /// Saves the world under the name typed.
    Names,
    /// Lists the worlds there are to open.
    Lists,
    /// Goes back to the options' first page.
    GoesBack,
    /// Opens a world, by its place among those listed.
    Opens(usize),
    /// Leaves Civil Egregore.
    Leaves,
}

/// What of the options is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    /// Their first page.
    First,
    /// The worlds there are to open.
    Worlds,
    /// A name for the world, typed.
    Naming,
}

/// Marks the options' parts.
#[derive(Component, Clone)]
pub struct Part;

/// The options: whether they are open, and what of them is shown.
#[derive(Resource)]
pub struct Options {
    /// Whether they are open.
    open: bool,
    /// What of them is shown.
    page: Page,
    /// The name of the world run, if it has one.
    named: Option<String>,
    /// The name being typed for it.
    typed: String,
    /// Whether the name typed was refused: a world there is has it.
    taken: bool,
    /// Names the worlds there are.
    lister: fn() -> Vec<String>,
    /// The worlds, as last listed.
    listing: Listing,
}

impl Options {
    /// The options, closed, their worlds named by `lister`.
    pub fn listing(lister: fn() -> Vec<String>) -> Self {
        Self { open: false, page: Page::First, named: None, typed: String::new(), taken: false, lister, listing: Listing::default() }
    }

    /// Whether they are open.
    pub fn open(&self) -> bool {
        self.open
    }

    /// The name of the world run, as they were told it.
    pub fn named(&self) -> Option<&str> {
        self.named.as_deref()
    }

    /// Tells them the name of the world run, if it has one: what it
    /// is saved under.
    pub fn name(&mut self, named: Option<String>) {
        self.named = named;
    }

    /// Says the world is to be saved under the name typed, and closes
    /// the options -- unless none is typed, or a world there is has it.
    fn save_named(&mut self, save: &mut MessageWriter<Save>) {
        let Some(name) = utilities::settings::world_name(&self.typed) else {
            return;
        };
        self.taken = (self.lister)().contains(&name);
        if !self.taken {
            save.write(Save(name));
            self.open = false;
        }
    }

    /// The rows shown, from the top.
    fn rows(&self) -> Vec<Row<Does>> {
        match self.page {
            Page::First => {
                let save = self.named.as_ref().map_or("save the world".to_string(), |named| format!("save {named}"));
                vec![Row::new("Civil Egregore", Look::Said, Does::Nothing), Row::new("go on (Escape)", Look::Clicked, Does::GoesOn), Row::new(save, Look::Clicked, Does::Saves), Row::new("open a world", Look::Clicked, Does::Lists), Row::new("exit", Look::Clicked, Does::Leaves)]
            }
            Page::Naming => {
                let taken = self.taken.then(|| Row::new("there is a world of that name", Look::Said, Does::Nothing));
                [Row::new("< options  |  a name for the world", Look::Back, Does::GoesBack), Row::new(format!("{}_", self.typed), Look::Said, Does::Nothing), Row::new("save (Enter)", Look::Clicked, Does::Names)].into_iter().chain(taken).collect()
            }
            Page::Worlds => self.listing.rows(Does::GoesBack, Does::Opens),
        }
    }
}

/// The options' parts, hidden.
pub fn spawn(mut commands: Commands) {
    rows::spawn(&mut commands, Part);
}

/// Opens and closes the options by Escape over a world -- unless a
/// slider's value is being typed -- does what a row clicked does,
/// takes the keys typing a name, goes through the worlds by the wheel,
/// and shows the rows as they are.
#[allow(clippy::too_many_arguments)]
pub fn work(
    mut options: ResMut<Options>,
    screen: Res<Screen>,
    sliders: Res<Sliders>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    mut shade: Single<&mut Visibility, (With<Shade>, With<Part>)>,
    mut bars: Query<(&Bar, &mut Node, &mut BackgroundColor), With<Part>>,
    mut words: Query<(&Says, &mut Text), With<Part>>,
    mut writers: (MessageWriter<AppExit>, MessageWriter<Open>, MessageWriter<Save>),
    mut typed: MessageReader<KeyboardInput>,
) {
    let (leave, open, save) = &mut writers;
    if *screen != Screen::World {
        options.open = false;
    } else if keys.just_pressed(KeyCode::Escape) && !sliders.typing() {
        // Opened on their first page.
        (options.open, options.page) = (!options.open, Page::First);
    } else if options.open {
        match rows::clicked(&options.rows(), &window, &buttons) {
            Some(Does::GoesOn) => options.open = false,
            Some(Does::Saves) => match options.named.clone() {
                Some(named) => {
                    save.write(Save(named));
                    options.open = false;
                }
                None => (options.page, options.typed, options.taken) = (Page::Naming, String::new(), false),
            },
            Some(Does::Names) => options.save_named(save),
            Some(Does::Lists) => (options.listing, options.page) = (Listing::of(options.lister), Page::Worlds),
            Some(Does::GoesBack) => options.page = Page::First,
            Some(Does::Opens(place)) => {
                open.write(Open(options.listing.worlds[place].clone()));
                options.open = false;
            }
            Some(Does::Leaves) => _ = leave.write(AppExit::Success),
            Some(Does::Nothing) | None => {}
        }
    }
    if options.open && options.page == Page::Naming {
        let mut name = std::mem::take(&mut options.typed);
        let entered = rows::type_into(&mut name, &mut typed, |letter| letter.is_alphanumeric() || "-_. ".contains(letter), NAME);
        options.typed = name;
        if entered {
            options.save_named(save);
        }
    } else {
        // Keys not typed into a name are no one's: none is kept for later.
        typed.clear();
    }
    if options.open && options.page == Page::Worlds {
        options.listing.scroll(wheel.delta.y);
    }
    rows::show(options.open, &options.rows(), &mut shade, &mut bars, &mut words);
}
