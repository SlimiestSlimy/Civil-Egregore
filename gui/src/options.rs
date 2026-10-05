//! The options, over the middle of the window: opened and closed by
//! Escape, a row each, clicked -- going on, opening a world, and
//! leaving TileSim.
//!
//! Opening a world lists those there are, as whoever added the menus
//! names them ([`crate::Gui::worlds`]), a row each, the wheel going
//! through more than fit: a click on one says it was chosen
//! ([`Chosen`]) and closes the options. Opening it is the window's.
//!
//! While they are open the view is not dragged and the wheel does not
//! zoom; the world ticks on behind them. Leaving is no more than
//! closing the window does: it is here to be found.

use crate::sliders::Hands;
use bevy::app::AppExit;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

/// Screen pixels the options are across.
const ACROSS: f32 = 420.0;
/// Screen pixels a row of the options is high.
const ROW: f32 = 48.0;
/// The height of the options' words, in screen pixels.
const WORDS: f32 = 20.0;
/// Worlds listed at a time: the wheel goes through the rest.
const LISTED: usize = 10;
/// Rows there are parts for: the most a page has.
const ROWS: usize = LISTED + 1;

/// What a click on a row of the options does.
#[derive(Clone, Copy)]
enum Does {
    /// Nothing: a row that only says something.
    Nothing,
    /// Closes the options.
    GoesOn,
    /// Lists the worlds there are to open.
    Lists,
    /// Goes back from the worlds to the options' first page.
    GoesBack,
    /// Chooses a world, by its place among those listed.
    Opens(usize),
    /// Leaves TileSim.
    Leaves,
}

/// A world chosen to be opened, by its name.
#[derive(Message)]
pub struct Chosen(pub String);

/// The options: whether they are open, and what of them is shown.
#[derive(Resource)]
pub struct Options {
    /// Whether they are open.
    open: bool,
    /// Whether the worlds are what is shown, not the first page.
    listing: bool,
    /// Names the worlds there are.
    lister: fn() -> Vec<String>,
    /// The worlds named, when they were last listed.
    worlds: Vec<String>,
    /// The first of them shown: how far the wheel has gone.
    first: usize,
}

impl Options {
    /// The options, closed, their worlds named by `lister`.
    pub fn listing(lister: fn() -> Vec<String>) -> Self {
        Self { open: false, listing: false, lister, worlds: Vec::new(), first: 0 }
    }

    /// Whether they are open: the pointer and the wheel are then theirs.
    pub fn open(&self) -> bool {
        self.open
    }

    /// The rows shown, from the top: what each says, and what a click
    /// on it does.
    fn rows(&self) -> Vec<(String, Does)> {
        if !self.listing {
            return [("TileSim", Does::Nothing), ("go on (Escape)", Does::GoesOn), ("open a world", Does::Lists), ("exit", Does::Leaves)].map(|(says, does)| (says.to_string(), does)).into();
        }
        let more = if self.worlds.len() > LISTED { ", the wheel for more" } else { "" };
        let mut rows = vec![(format!("< options  |  {} world(s){more}", self.worlds.len()), Does::GoesBack)];
        rows.extend(self.worlds.iter().enumerate().skip(self.first).take(LISTED).map(|(place, world)| (world.clone(), Does::Opens(place))));
        if self.worlds.is_empty() {
            rows.push(("none in the worlds' folder".to_string(), Does::Nothing));
        }
        rows
    }
}

/// The options, all of them: the window dimmed, shown and hidden.
#[derive(Component)]
pub struct Shade;

/// The `.0`-th row's bar, from the top.
#[derive(Component)]
pub struct Bar(usize);

/// The `.0`-th row's words.
#[derive(Component)]
pub struct Says(usize);

/// The options, hidden: the window dimmed, and the rows in its middle.
pub fn setup(mut commands: Commands) {
    let whole = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
    commands.spawn((whole, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)), GlobalZIndex(10), Visibility::Hidden, Shade)).with_children(|rows| {
        for row in 0..ROWS {
            let bar = Node { width: Val::Px(ACROSS), height: Val::Px(ROW), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() };
            rows.spawn((bar, BackgroundColor(Color::BLACK), Bar(row))).with_child((Text::new(""), TextFont { font_size: FontSize::Px(WORDS), ..default() }, Says(row)));
        }
    });
}

/// Which of `rows` rows the pointer is on, if it is on one: they are
/// in the window's middle, a row under another.
fn row_under(window: &Window, rows: usize) -> Option<usize> {
    let pointer = window.cursor_position()?;
    let (left, top) = ((window.width() - ACROSS) / 2.0, (window.height() - ROW * rows as f32) / 2.0);
    let within = (left..left + ACROSS).contains(&pointer.x) && pointer.y >= top;
    within.then_some(((pointer.y - top) / ROW) as usize).filter(|&row| row < rows)
}

/// Opens and closes the options by Escape -- unless it is leaving a
/// value being typed -- does what a row clicked does, goes through
/// the worlds by the wheel, and shows the rows as they are.
pub fn work(
    mut options: ResMut<Options>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    hands: Res<Hands>,
    window: Single<&Window>,
    mut shade: Single<&mut Visibility, With<Shade>>,
    mut bars: Query<(&Bar, &mut Node, &mut BackgroundColor)>,
    mut words: Query<(&Says, &mut Text)>,
    mut leave: MessageWriter<AppExit>,
    mut chosen: MessageWriter<Chosen>,
) {
    if keys.just_pressed(KeyCode::Escape) && !hands.typing() {
        // Opened on their first page.
        (options.open, options.listing) = (!options.open, false);
    } else if options.open && buttons.just_pressed(MouseButton::Left) {
        let rows = options.rows();
        match row_under(&window, rows.len()).map(|row| rows[row].1) {
            Some(Does::GoesOn) => options.open = false,
            Some(Does::Lists) => {
                let worlds = (options.lister)();
                (options.worlds, options.first, options.listing) = (worlds, 0, true);
            }
            Some(Does::GoesBack) => options.listing = false,
            Some(Does::Opens(place)) => {
                chosen.write(Chosen(options.worlds[place].clone()));
                options.open = false;
            }
            Some(Does::Leaves) => _ = leave.write(AppExit::Success),
            Some(Does::Nothing) | None => {}
        }
    }
    shade.set_if_neq(if options.open { Visibility::Visible } else { Visibility::Hidden });
    if !options.open {
        return;
    }
    if options.listing && wheel.delta.y != 0.0 {
        // A notch a world, no further than the last ones filling the list.
        let last = options.worlds.len().saturating_sub(LISTED) as f32;
        options.first = (options.first as f32 - wheel.delta.y.signum()).clamp(0.0, last) as usize;
    }
    let rows = options.rows();
    for (bar, mut node, mut colour) in &mut bars {
        let (display, shade) = match rows.get(bar.0) {
            None => (Display::None, Color::BLACK),
            Some((_, Does::Nothing)) => (Display::Flex, Color::srgb(0.08, 0.08, 0.08)),
            Some((_, Does::GoesBack)) => (Display::Flex, Color::srgb(0.3, 0.3, 0.3)),
            Some(_) if bar.0 % 2 == 1 => (Display::Flex, Color::srgb(0.22, 0.22, 0.22)),
            Some(_) => (Display::Flex, Color::srgb(0.17, 0.17, 0.17)),
        };
        if node.display != display {
            node.display = display;
        }
        if colour.0 != shade {
            colour.0 = shade;
        }
    }
    for (says, mut text) in &mut words {
        let said = rows.get(says.0).map_or("", |(said, _)| said.as_str());
        if text.0 != said {
            text.0 = said.to_string();
        }
    }
}
