//! What the window opens on, before any world: a new world, a world
//! saved, or leaving -- a row each ([`crate::rows`]).
//!
//! - **A new world** is set up on a page of its own: its seed typed in
//!   hexadecimal, or none for one drawn at random, and the rest as the
//!   sliders have it -- every group of them offered while it is set up,
//!   `U` opening them: the world's size, whether it is forced hot, its
//!   sheep, and how it is generated. Made, it is said ([`Make`]).
//! - **A world saved** is chosen from those there are
//!   ([`crate::Gui::worlds`]), a row each, the wheel going through more
//!   than fit: chosen, it is said ([`Open`]).
//!
//! Either way the window then shows the world ([`Screen::World`]).
//! Escape goes back a page.

use crate::rows::{self, Bar, Listing, Look, Row, Says, Shade};
use crate::sliders::Sliders;
use crate::{CurrentTuning, Make, Open, Screen};
use bevy::app::AppExit;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// What a click on a row of the main menu does.
#[derive(Clone, Copy)]
enum Does {
    /// Nothing: a row that only says something.
    Nothing,
    /// Sets up a new world.
    SetsUp,
    /// Makes the world set up.
    Makes,
    /// Lists the worlds there are to open.
    Lists,
    /// Goes back to the first page.
    GoesBack,
    /// Opens a world, by its place among those listed.
    Opens(usize),
    /// Leaves Civil Egregore.
    Leaves,
}

/// What of the main menu is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    /// Its first page.
    First,
    /// A new world being set up.
    New,
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
    page: Page,
    /// The new world's seed, typed: hexadecimal digits, or none.
    seed: String,
    /// Names the worlds there are.
    lister: fn() -> Vec<String>,
    /// The worlds, as last listed.
    listing: Listing,
}

impl MainMenu {
    /// The main menu, on its first page, its worlds named by `lister`.
    pub fn listing(lister: fn() -> Vec<String>) -> Self {
        Self { page: Page::First, seed: String::new(), lister, listing: Listing::default() }
    }

    /// Whether a new world is being set up: every slider is offered.
    pub fn setting_up(&self) -> bool {
        self.page == Page::New
    }

    /// The seed typed, if one is.
    fn seed(&self) -> Option<u64> {
        utilities::seed::of_hex(&self.seed)
    }

    /// The rows shown, from the top.
    fn rows(&self) -> Vec<Row<Does>> {
        match self.page {
            Page::First => vec![Row::new("Civil Egregore", Look::Said, Does::Nothing), Row::new("new world", Look::Clicked, Does::SetsUp), Row::new("open a world", Look::Clicked, Does::Lists), Row::new("exit", Look::Clicked, Does::Leaves)],
            Page::New => vec![
                Row::new("< back  |  a new world", Look::Back, Does::GoesBack),
                Row::new(format!("seed 0x{}_", self.seed), Look::Said, Does::Nothing),
                Row::new(if self.seed.is_empty() { "none typed: one drawn at random" } else { "in hexadecimal" }, Look::Said, Does::Nothing),
                Row::new("U: its size, its sheep, its land", Look::Said, Does::Nothing),
                Row::new("make the world (Enter)", Look::Clicked, Does::Makes),
            ],
            Page::Worlds => self.listing.rows(Does::GoesBack, Does::Opens),
        }
    }
}

/// The main menu's parts, hidden until shown.
pub fn spawn(mut commands: Commands) {
    rows::spawn(&mut commands, Part);
}

/// Does what a row clicked does -- unless the pointer is the sliders'
/// -- takes the keys typing a seed, goes through the worlds by the
/// wheel, goes back a page by Escape, and shows the rows as they are:
/// all while the window shows it.
#[allow(clippy::too_many_arguments)]
pub fn work(
    mut menu: ResMut<MainMenu>,
    mut screen: ResMut<Screen>,
    sliders: Res<Sliders>,
    tuning: Res<CurrentTuning>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    mut shade: Single<&mut Visibility, (With<Shade>, With<Part>)>,
    mut bars: Query<(&Bar, &mut Node, &mut BackgroundColor), With<Part>>,
    mut words: Query<(&Says, &mut Text), With<Part>>,
    mut writers: (MessageWriter<AppExit>, MessageWriter<Make>, MessageWriter<Open>),
    mut typed: MessageReader<KeyboardInput>,
) {
    let (leave, make, open) = &mut writers;
    let shown = *screen == Screen::MainMenu;
    let mut makes = false;
    if shown && !sliders.typing() && keys.just_pressed(KeyCode::Escape) {
        menu.page = Page::First;
    }
    if shown && !sliders.held() && !sliders.over(&window) {
        match rows::clicked(&menu.rows(), &window, &buttons) {
            Some(Does::SetsUp) => menu.page = Page::New,
            Some(Does::Makes) => makes = true,
            Some(Does::Lists) => (menu.listing, menu.page) = (Listing::of(menu.lister), Page::Worlds),
            Some(Does::GoesBack) => menu.page = Page::First,
            Some(Does::Opens(place)) => {
                open.write(Open(menu.listing.worlds[place].clone()));
                *screen = Screen::World;
            }
            Some(Does::Leaves) => _ = leave.write(AppExit::Success),
            Some(Does::Nothing) | None => {}
        }
    }
    if shown && menu.page == Page::New && !sliders.typing() {
        let mut seed = std::mem::take(&mut menu.seed);
        makes |= rows::type_into(&mut seed, &mut typed, |digit| digit.is_ascii_hexdigit(), 16);
        menu.seed = seed;
    } else {
        // Keys not typed into a seed are no one's: none is kept for later.
        typed.clear();
    }
    if makes {
        make.write(Make { seed: menu.seed(), tuning: tuning.0 });
        *screen = Screen::World;
    }
    if shown && menu.page == Page::Worlds && !sliders.over(&window) {
        menu.listing.scroll(wheel.delta.y);
    }
    let shown = *screen == Screen::MainMenu;
    rows::show(shown, &menu.rows(), &mut shade, &mut bars, &mut words);
}
