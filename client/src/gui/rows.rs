//! A menu of rows over the middle of the window -- what the main menu
//! and the options both are -- with the worlds listed and keys typed
//! into a line (`docs/gui.md`, "Rows").

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

/// Screen pixels the rows are across.
const ROW_WIDTH_PIXELS: f32 = 420.0;
/// Screen pixels a row is high.
const ROW_HEIGHT_PIXELS: f32 = 48.0;
/// The height of the rows' words, in screen pixels.
const WORDS: f32 = 20.0;
/// Worlds listed at a time: the wheel goes through the rest.
pub const WORLDS_LISTED_AT_ONCE: usize = 10;
/// Rows there are parts for: the most a page has.
pub const MOST_ROWS: usize = WORLDS_LISTED_AT_ONCE + 2;

/// How a row looks, and whether a click on it does anything.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowLook {
    /// It only says something.
    OnlySays,
    /// It goes back a page.
    GoesBack,
    /// A click on it does something.
    ActsOnClick,
}

/// A row: what it says, how it looks, and what a click on it does --
/// `ClickDoes` the menu's own.
pub struct MenuRow<ClickDoes> {
    /// Its words.
    pub row_text: String,
    /// How it looks.
    pub row_look: RowLook,
    /// What a click does.
    pub click_does: ClickDoes,
}

impl<ClickDoes> MenuRow<ClickDoes> {
    /// A row saying `row_text`, looking `row_look`, doing `click_does`.
    pub fn new(row_text: impl Into<String>, row_look: RowLook, click_does: ClickDoes) -> Self {
        Self { row_text: row_text.into(), row_look, click_does }
    }
}

/// A menu's parts, all of them: the window dimmed, shown and hidden.
#[derive(Component)]
pub struct MenuShade;

/// The `.0`-th row's bar, from the top.
#[derive(Component)]
pub struct Bar(usize);

/// The `.0`-th row's words.
#[derive(Component)]
pub struct RowText(usize);

/// A menu's parts, hidden, each marked `part_marker`: the window dimmed,
/// and the rows in its middle.
pub fn spawn_menu_parts(bevy_commands: &mut Commands, part_marker: impl Component + Clone) {
    let whole_window_node = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
    bevy_commands.spawn((whole_window_node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)), GlobalZIndex(10), Visibility::Hidden, MenuShade, part_marker.clone())).with_children(|row_spawner| {
        for row_number in 0..MOST_ROWS {
            let bar = Node { width: Val::Px(ROW_WIDTH_PIXELS), height: Val::Px(ROW_HEIGHT_PIXELS), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
            row_spawner.spawn((bar, BackgroundColor(Color::BLACK), Bar(row_number), part_marker.clone())).with_child((Text::new(""), TextFont { font_size: FontSize::Px(WORDS), ..default() }, RowText(row_number), part_marker.clone()));
        }
    });
}

/// Which of `row_count` rows the pointer is on, if it is on one: they are
/// in the window's middle, a row under another.
pub fn row_under_pointer(window: &Window, row_count: usize) -> Option<usize> {
    let pointer_position = window.cursor_position()?;
    let (rows_left_pixel, rows_top_pixel) = ((window.width() - ROW_WIDTH_PIXELS) / 2.0, (window.height() - ROW_HEIGHT_PIXELS * row_count as f32) / 2.0);
    let pointer_within = (rows_left_pixel..rows_left_pixel + ROW_WIDTH_PIXELS).contains(&pointer_position.x) && pointer_position.y >= rows_top_pixel;
    pointer_within.then_some(((pointer_position.y - rows_top_pixel) / ROW_HEIGHT_PIXELS) as usize).filter(|&row_number| row_number < row_count)
}

/// What the row clicked does, if the left button was just pressed on
/// one of `menu_rows` that does something.
pub fn clicked_row_does<ClickDoes: Copy>(menu_rows: &[MenuRow<ClickDoes>], window: &Window, mouse_buttons: &ButtonInput<MouseButton>) -> Option<ClickDoes> {
    let row_number = row_under_pointer(window, menu_rows.len()).filter(|_| mouse_buttons.just_pressed(MouseButton::Left))?;
    (menu_rows[row_number].row_look != RowLook::OnlySays).then_some(menu_rows[row_number].click_does)
}

/// A menu's parts, as `menu_rows` say, if it `menu_open`s: each bar shown or not
/// and coloured as its row looks, and its words.
pub fn show_rows<ClickDoes, PartFilter: QueryFilter>(menu_open: bool, menu_rows: &[MenuRow<ClickDoes>], shade_visibility: &mut Visibility, bars: &mut Query<(&Bar, &mut Node, &mut BackgroundColor), PartFilter>, words: &mut Query<(&RowText, &mut Text), PartFilter>) {
    let wanted_visibility = if menu_open { Visibility::Visible } else { Visibility::Hidden };
    if *shade_visibility != wanted_visibility {
        *shade_visibility = wanted_visibility;
    }
    if !menu_open {
        return;
    }
    for (bar, mut node, mut bar_background) in bars.iter_mut() {
        let (bar_display, bar_colour) = match menu_rows.get(bar.0).map(|menu_row| menu_row.row_look) {
            None => (Display::None, Color::BLACK),
            Some(RowLook::OnlySays) => (Display::Flex, Color::srgb(0.08, 0.08, 0.08)),
            Some(RowLook::GoesBack) => (Display::Flex, Color::srgb(0.3, 0.3, 0.3)),
            Some(RowLook::ActsOnClick) if bar.0 % 2 == 1 => (Display::Flex, Color::srgb(0.22, 0.22, 0.22)),
            Some(RowLook::ActsOnClick) => (Display::Flex, Color::srgb(0.17, 0.17, 0.17)),
        };
        if node.display != bar_display {
            node.display = bar_display;
        }
        if bar_background.0 != bar_colour {
            bar_background.0 = bar_colour;
        }
    }
    for (text_marker, mut shown_text) in words.iter_mut() {
        let wanted_text = menu_rows.get(text_marker.0).map_or("", |menu_row| menu_row.row_text.as_str());
        if shown_text.0 != wanted_text {
            shown_text.0 = wanted_text.to_string();
        }
    }
}

/// The worlds there are to open, as last listed, and the first of them
/// shown: how far the wheel has gone.
#[derive(Default)]
pub struct WorldListing {
    /// The worlds' names.
    pub worlds: Vec<String>,
    /// The first of them shown.
    first_shown: usize,
}

impl WorldListing {
    /// The worlds `world_lister` names, from the first.
    pub fn from_lister(world_lister: &crate::gui::WorldLister) -> Self {
        Self { worlds: (world_lister.0)(), first_shown: 0 }
    }

    /// The rows listing them -- under a first that goes back, `does`
    /// for each world by its place among them -- or one saying there
    /// are none.
    pub fn world_rows<ClickDoes>(&self, back_does: ClickDoes, opens_world: impl Fn(usize) -> ClickDoes) -> Vec<MenuRow<ClickDoes>> {
        let more_hint = if self.worlds.len() > WORLDS_LISTED_AT_ONCE { ", the wheel for more" } else { "" };
        let mut listed_rows = vec![MenuRow::new(format!("< back  |  {} world(s){more_hint}", self.worlds.len()), RowLook::GoesBack, back_does)];
        listed_rows.extend(self.worlds.iter().enumerate().skip(self.first_shown).take(WORLDS_LISTED_AT_ONCE).map(|(place, world)| MenuRow::new(world.clone(), RowLook::ActsOnClick, opens_world(place))));
        if self.worlds.is_empty() {
            listed_rows.push(MenuRow::new("none in the worlds' folder", RowLook::OnlySays, opens_world(0)));
        }
        listed_rows
    }

    /// Goes through the worlds by the wheel: a notch a world, no
    /// further than the last ones filling the list.
    pub fn scroll_by_wheel(&mut self, wheel: f32) {
        if wheel != 0.0 {
            let highest_first_shown = self.worlds.len().saturating_sub(WORLDS_LISTED_AT_ONCE) as f32;
            self.first_shown = (self.first_shown as f32 - wheel.signum()).clamp(0.0, highest_first_shown) as usize;
        }
    }
}

/// Types the keys pressed into `line`, each letter `letter_allowed` keeps, up
/// to `most_letters` letters, Backspace taking the last off: whether Enter was
/// pressed.
pub fn type_into(line: &mut String, typed_keys: &mut MessageReader<KeyboardInput>, letter_allowed: impl Fn(char) -> bool, most_letters: usize) -> bool {
    let mut enter_pressed = false;
    for pressed_key in typed_keys.read().filter(|key_event| key_event.state.is_pressed()) {
        match &pressed_key.logical_key {
            Key::Character(typed_letters) => {
                let letters_room = most_letters.saturating_sub(line.chars().count());
                line.extend(typed_letters.chars().filter(|&typed_letter| letter_allowed(typed_letter)).take(letters_room));
            }
            Key::Space if letter_allowed(' ') && !line.is_empty() && line.chars().count() < most_letters => line.push(' '),
            Key::Backspace => _ = line.pop(),
            Key::Enter => enter_pressed = true,
            _ => {}
        }
    }
    enter_pressed
}
