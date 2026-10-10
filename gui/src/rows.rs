//! A menu of rows over the middle of the window -- what the main menu
//! and the options both are -- with the worlds listed and keys typed
//! into a line (`docs/gui.md`, "Rows").

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

/// Screen pixels the rows are across.
const ACROSS: f32 = 420.0;
/// Screen pixels a row is high.
const HIGH: f32 = 48.0;
/// The height of the rows' words, in screen pixels.
const WORDS: f32 = 20.0;
/// Worlds listed at a time: the wheel goes through the rest.
pub const LISTED: usize = 10;
/// Rows there are parts for: the most a page has.
pub const ROWS: usize = LISTED + 2;

/// How a row looks, and whether a click on it does anything.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// It only says something.
    Said,
    /// It goes back a page.
    Back,
    /// A click on it does something.
    Clicked,
}

/// A row: what it says, how it looks, and what a click on it does --
/// `Does` the menu's own.
pub struct Row<Does> {
    /// Its words.
    pub says: String,
    /// How it looks.
    pub look: Look,
    /// What a click does.
    pub does: Does,
}

impl<Does> Row<Does> {
    /// A row saying `says`, looking `look`, doing `does`.
    pub fn new(says: impl Into<String>, look: Look, does: Does) -> Self {
        Self { says: says.into(), look, does }
    }
}

/// A menu's parts, all of them: the window dimmed, shown and hidden.
#[derive(Component)]
pub struct Shade;

/// The `.0`-th row's bar, from the top.
#[derive(Component)]
pub struct Bar(usize);

/// The `.0`-th row's words.
#[derive(Component)]
pub struct Says(usize);

/// A menu's parts, hidden, each marked `marker`: the window dimmed,
/// and the rows in its middle.
pub fn spawn(commands: &mut Commands, marker: impl Component + Clone) {
    let whole = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
    commands.spawn((whole, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)), GlobalZIndex(10), Visibility::Hidden, Shade, marker.clone())).with_children(|rows| {
        for row in 0..ROWS {
            let bar = Node { width: Val::Px(ACROSS), height: Val::Px(HIGH), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
            rows.spawn((bar, BackgroundColor(Color::BLACK), Bar(row), marker.clone())).with_child((Text::new(""), TextFont { font_size: FontSize::Px(WORDS), ..default() }, Says(row), marker.clone()));
        }
    });
}

/// Which of `rows` rows the pointer is on, if it is on one: they are
/// in the window's middle, a row under another.
pub fn under(window: &Window, rows: usize) -> Option<usize> {
    let pointer = window.cursor_position()?;
    let (left, top) = ((window.width() - ACROSS) / 2.0, (window.height() - HIGH * rows as f32) / 2.0);
    let within = (left..left + ACROSS).contains(&pointer.x) && pointer.y >= top;
    within.then_some(((pointer.y - top) / HIGH) as usize).filter(|&row| row < rows)
}

/// What the row clicked does, if the left button was just pressed on
/// one of `rows` that does something.
pub fn clicked<Does: Copy>(rows: &[Row<Does>], window: &Window, buttons: &ButtonInput<MouseButton>) -> Option<Does> {
    let row = under(window, rows.len()).filter(|_| buttons.just_pressed(MouseButton::Left))?;
    (rows[row].look != Look::Said).then_some(rows[row].does)
}

/// A menu's parts, as `rows` say, if it `open`s: each bar shown or not
/// and coloured as its row looks, and its words.
pub fn show<Does, Of: QueryFilter>(open: bool, rows: &[Row<Does>], shade: &mut Visibility, bars: &mut Query<(&Bar, &mut Node, &mut BackgroundColor), Of>, words: &mut Query<(&Says, &mut Text), Of>) {
    let shown = if open { Visibility::Visible } else { Visibility::Hidden };
    if *shade != shown {
        *shade = shown;
    }
    if !open {
        return;
    }
    for (bar, mut node, mut colour) in bars.iter_mut() {
        let (display, shade) = match rows.get(bar.0).map(|row| row.look) {
            None => (Display::None, Color::BLACK),
            Some(Look::Said) => (Display::Flex, Color::srgb(0.08, 0.08, 0.08)),
            Some(Look::Back) => (Display::Flex, Color::srgb(0.3, 0.3, 0.3)),
            Some(Look::Clicked) if bar.0 % 2 == 1 => (Display::Flex, Color::srgb(0.22, 0.22, 0.22)),
            Some(Look::Clicked) => (Display::Flex, Color::srgb(0.17, 0.17, 0.17)),
        };
        if node.display != display {
            node.display = display;
        }
        if colour.0 != shade {
            colour.0 = shade;
        }
    }
    for (says, mut text) in words.iter_mut() {
        let said = rows.get(says.0).map_or("", |row| row.says.as_str());
        if text.0 != said {
            text.0 = said.to_string();
        }
    }
}

/// The worlds there are to open, as last listed, and the first of them
/// shown: how far the wheel has gone.
#[derive(Default)]
pub struct Listing {
    /// The worlds' names.
    pub worlds: Vec<String>,
    /// The first of them shown.
    first: usize,
}

impl Listing {
    /// The worlds `lister` names, from the first.
    pub fn of(lister: fn() -> Vec<String>) -> Self {
        Self { worlds: lister(), first: 0 }
    }

    /// The rows listing them -- under a first that goes back, `does`
    /// for each world by its place among them -- or one saying there
    /// are none.
    pub fn rows<Does>(&self, back: Does, opens: impl Fn(usize) -> Does) -> Vec<Row<Does>> {
        let more = if self.worlds.len() > LISTED { ", the wheel for more" } else { "" };
        let mut rows = vec![Row::new(format!("< back  |  {} world(s){more}", self.worlds.len()), Look::Back, back)];
        rows.extend(self.worlds.iter().enumerate().skip(self.first).take(LISTED).map(|(place, world)| Row::new(world.clone(), Look::Clicked, opens(place))));
        if self.worlds.is_empty() {
            rows.push(Row::new("none in the worlds' folder", Look::Said, opens(0)));
        }
        rows
    }

    /// Goes through the worlds by the wheel: a notch a world, no
    /// further than the last ones filling the list.
    pub fn scroll(&mut self, wheel: f32) {
        if wheel != 0.0 {
            let last = self.worlds.len().saturating_sub(LISTED) as f32;
            self.first = (self.first as f32 - wheel.signum()).clamp(0.0, last) as usize;
        }
    }
}

/// Types the keys pressed into `line`, each letter `allowed` keeps, up
/// to `most` letters, Backspace taking the last off: whether Enter was
/// pressed.
pub fn type_into(line: &mut String, typed: &mut MessageReader<KeyboardInput>, allowed: impl Fn(char) -> bool, most: usize) -> bool {
    let mut entered = false;
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        match &key.logical_key {
            Key::Character(letters) => {
                let room = most.saturating_sub(line.chars().count());
                line.extend(letters.chars().filter(|&letter| allowed(letter)).take(room));
            }
            Key::Space if allowed(' ') && !line.is_empty() && line.chars().count() < most => line.push(' '),
            Key::Backspace => _ = line.pop(),
            Key::Enter => entered = true,
            _ => {}
        }
    }
    entered
}
