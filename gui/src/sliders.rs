//! Sliders over the window's top right corner, one a number of
//! [`crate::tuning`], in groups ([`Group`]), one group on the screen at
//! a time, opened from a menu.
//!
//! - **Closed**, there is one small button, `sliders`: a click on it,
//!   or `U`, opens the menu.
//! - **The menu** lists the groups, a row each: a click on one opens
//!   it. In the lab, a last row draws the world's seed again. A click
//!   on its first row, or `U`, closes it.
//! - **A group** is its sliders, under a first row that goes back to
//!   the menu: a knob dragged with the left button, set back to its
//!   default with the right, and kept when let go. Beside each, a box
//!   with its value: a click on it, and the value is typed -- digits
//!   and a point, Enter to set it, Escape to leave it. The pointer
//!   rested on a slider's row for a moment, and what the slider does is
//!   said beside it.
//!
//! The groups that say how the world is generated are listed only in
//! the lab, where they are read. What is longer than the window is
//! scrolled by the wheel, the pointer over it.
//!
//! All is laid out by plain arithmetic -- a row each, a track of a
//! fixed width against the window's right edge -- so where the pointer
//! is on one is worked out from the same numbers, with no asking Bevy.

use crate::tuning::{keep, now, reseed, set, tuned, unless_set, Group, GROUPS, NAMES};
use bevy::prelude::*;
use bevy::input::mouse::AccumulatedMouseScroll;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

/// Screen pixels from the window's top and right edges to the sliders.
const MARGIN: f32 = 16.0;
/// Screen pixels from one slider to the next.
const ROW: f32 = 36.0;
/// Screen pixels a slider's track is across, and high.
const TRACK: (f32, f32) = (300.0, 8.0);
/// Screen pixels a slider's knob is across, and high.
const KNOB: (f32, f32) = (14.0, 26.0);
/// Screen pixels a value's box is across, and high.
const BOX: (f32, f32) = (84.0, 26.0);
/// Screen pixels between a slider's name, its box and its track.
const GAP: f32 = 14.0;
/// Screen pixels the sliders, their boxes and their names take across.
const PANEL: f32 = TRACK.0 + GAP + BOX.0 + GAP + 170.0;
/// The height of a slider's name and value, in screen pixels.
const NAME: f32 = 18.0;
/// Screen pixels the button shown while the sliders are closed is across.
const CLOSED: f32 = 96.0;

/// Seconds the pointer rests on a row before what its slider does is said.
const REST: f32 = 0.4;
/// Screen pixels across what a slider does is said in, at most.
const TIP: f32 = 380.0;

/// Screen pixels a notch of the wheel scrolls a page.
const NOTCH: f32 = 2.0 * ROW;

/// Screen pixels the page shown is scrolled up by: an `f32`'s bits.
static SCROLLED: AtomicU32 = AtomicU32::new(0);

/// Screen pixels the page shown is scrolled up by.
fn scrolled() -> f32 {
    f32::from_bits(SCROLLED.load(Ordering::Relaxed))
}

/// Whether the left button went down over the sliders and is still
/// held: the view is then not dragged, wherever the pointer goes.
static HELD: AtomicBool = AtomicBool::new(false);

/// What of the sliders is on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Only the button that opens the menu.
    Closed,
    /// The menu: the groups, a row each.
    Menu,
    /// One group's sliders.
    Group(Group),
}

/// What is shown: 0 closed, 1 the menu, else two more than a group's
/// place in [`GROUPS`].
static SHOWN: AtomicU8 = AtomicU8::new(0);

/// Whether the lab runs: the groups of generation are then listed.
static IN_LAB: AtomicBool = AtomicBool::new(false);

/// What is shown.
fn shown() -> Shown {
    match SHOWN.load(Ordering::Relaxed) as usize {
        0 => Shown::Closed,
        1 => Shown::Menu,
        group => Shown::Group(GROUPS[group - 2]),
    }
}

/// Shows `what`, from its top.
fn show(what: Shown) {
    let number = match what {
        Shown::Closed => 0,
        Shown::Menu => 1,
        Shown::Group(group) => 2 + GROUPS.iter().position(|&listed| listed == group).expect("a group of the groups"),
    };
    SHOWN.store(number as u8, Ordering::Relaxed);
    SCROLLED.store(0f32.to_bits(), Ordering::Relaxed);
}

/// Anything of the sliders, and what it is shown with.
#[derive(Component)]
pub struct Part(Shown);

/// How far down the window a part is, not scrolled.
#[derive(Component)]
pub struct Row(f32);

/// A part of a slider that moves with its number.
#[derive(Component)]
pub struct Moved {
    /// The number's place in [`NAMES`].
    index: usize,
    /// Whether it is the knob, or the filled part of the track.
    knob: bool,
}

/// The value of the `.0`-th number, in its box.
#[derive(Component)]
pub struct Valued(usize);

/// Where what a slider does is said.
#[derive(Component)]
pub struct Tip;

/// What the pointer and the keys are doing with the sliders.
#[derive(Resource, Default)]
pub struct Hands {
    /// The slider being dragged, if one is.
    dragged: Option<usize>,
    /// The number whose value is being typed, and what is typed so far.
    typed: Option<(usize, String)>,
    /// Where the pointer last was, and the seconds it has rested there.
    rested: (Vec2, f32),
}

impl Hands {
    /// Whether a value is being typed.
    pub fn typing(&self) -> bool {
        self.typed.is_some()
    }
}

/// The group shown, if one is.
fn page() -> Option<Group> {
    match shown() {
        Shown::Group(group) => Some(group),
        _ => None,
    }
}

/// The numbers of `group`, by their places in [`NAMES`], a row each.
fn rows(group: Group) -> impl Iterator<Item = usize> {
    (0..NAMES.len()).filter(move |&index| tuned(index).group == group)
}

/// The groups the menu lists: those of generation only in the lab.
fn listed() -> impl Iterator<Item = Group> {
    GROUPS.into_iter().filter(|group| !group.generation() || IN_LAB.load(Ordering::Relaxed))
}

/// The lab runs: the menu lists every group, and is open from the start.
pub fn in_lab() {
    IN_LAB.store(true, Ordering::Relaxed);
    show(Shown::Menu);
}

/// Whether the sliders have the left button: it went down over them.
pub fn held() -> bool {
    HELD.load(Ordering::Relaxed)
}

/// How many rows what is shown has under its first: the menu's groups
/// and, in the lab, its button; a group's sliders.
fn rows_under() -> usize {
    match shown() {
        Shown::Closed => 0,
        Shown::Menu => listed().count() + IN_LAB.load(Ordering::Relaxed) as usize,
        Shown::Group(group) => rows(group).count(),
    }
}

/// The middle of the `row`-th row, the first 0, from the window's top.
fn middle(row: usize) -> f32 {
    MARGIN + ROW * (row as f32 + 0.5) - 4.0
}

/// Where what is shown ends, from the window's top, not scrolled.
fn foot() -> f32 {
    MARGIN + ROW * (1 + rows_under()) as f32 + 4.0
}

/// Screen pixels what is shown takes across.
fn across() -> f32 {
    if shown() == Shown::Closed { CLOSED } else { PANEL }
}

/// Whether the pointer is over what is shown.
pub fn over(window: &Window) -> bool {
    window.cursor_position().is_some_and(|pointer| pointer.x >= window.width() - MARGIN - across() && pointer.y <= foot() + MARGIN - scrolled())
}

/// The row the pointer is on, the first 0, if it is over what is shown.
fn row_under(window: &Window) -> Option<usize> {
    let pointer = window.cursor_position().filter(|_| over(window))?;
    let down = pointer.y + scrolled() - MARGIN;
    (down >= 0.0).then_some((down / ROW) as usize).filter(|&row| row <= rows_under())
}

/// Scrolls what is shown by the wheel, the pointer over it: no
/// further than its last row at the window's foot. And shows it, of
/// all the parts.
pub fn scroll(wheel: Res<AccumulatedMouseScroll>, window: Single<&Window>, mut parts: Query<(&Part, &Row, &mut Node, &mut Visibility)>) {
    let most = (foot() + MARGIN - window.height()).max(0.0);
    let moved = if over(&window) { wheel.delta.y * NOTCH } else { 0.0 };
    let now = (scrolled() - moved).clamp(0.0, most);
    if now != scrolled() {
        SCROLLED.store(now.to_bits(), Ordering::Relaxed);
    }
    for (part, row, mut node, mut visibility) in &mut parts {
        visibility.set_if_neq(if part.0 == shown() { Visibility::Visible } else { Visibility::Hidden });
        if node.top != Val::Px(row.0 - now) {
            node.top = Val::Px(row.0 - now);
        }
    }
}

/// The button, the menu and every group: each a dark panel, a first
/// row saying what a click on it does, and under it the menu's groups
/// or the group's sliders -- a name, a box with the value, a track, its
/// filled part and a knob each.
pub fn setup(mut commands: Commands) {
    let placed = |top: f32, right: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top), right: Val::Px(right), width, height: Val::Px(height), ..default() };
    let text = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME), ..default() });
    let within = |row: usize, height: f32| middle(row) - height / 2.0;
    // Bevy stacks them by `ZIndex`, not by the order they are made in: the panel under all, the knobs over all.
    let panel = |commands: &mut Commands, what: Shown, across: f32, rows: usize| {
        commands.spawn((placed(MARGIN - 8.0, MARGIN - 12.0, Val::Px(across + 12.0), ROW * rows as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), ZIndex(0), Visibility::Hidden, Part(what), Row(MARGIN - 8.0)));
    };
    // A row that is clicked: a bar, and what the click does.
    let bar = |commands: &mut Commands, what: Shown, row: usize, across: f32, words: &str, colour: Color| {
        commands.spawn((placed(within(row, ROW - 8.0), MARGIN, Val::Px(across - 12.0), ROW - 8.0), BackgroundColor(colour), ZIndex(1), Visibility::Hidden, Part(what), Row(within(row, ROW - 8.0))));
        commands.spawn((text(words), placed(within(row, NAME * 1.2), MARGIN + 10.0, Val::Px(across - 32.0), NAME * 1.2), ZIndex(2), Visibility::Hidden, Part(what), Row(within(row, NAME * 1.2))));
    };
    let (head, entry, button) = (Color::srgb(0.22, 0.22, 0.22), Color::srgb(0.14, 0.14, 0.14), Color::srgb(0.2, 0.35, 0.6));
    panel(&mut commands, Shown::Closed, CLOSED, 1);
    bar(&mut commands, Shown::Closed, 0, CLOSED, "sliders", head);
    let groups: Vec<Group> = listed().collect();
    let in_lab = IN_LAB.load(Ordering::Relaxed);
    panel(&mut commands, Shown::Menu, PANEL, 1 + groups.len() + in_lab as usize);
    bar(&mut commands, Shown::Menu, 0, PANEL, "sliders: a group to open, here or U to close", head);
    for (row, group) in groups.iter().enumerate() {
        bar(&mut commands, Shown::Menu, 1 + row, PANEL, group.name(), entry);
    }
    if in_lab {
        bar(&mut commands, Shown::Menu, 1 + groups.len(), PANEL, "regenerate, a random seed", button);
    }
    for group in groups {
        let what = Shown::Group(group);
        panel(&mut commands, what, PANEL, 1 + rows(group).count());
        bar(&mut commands, what, 0, PANEL, &format!("< groups  |  {}", group.name()), head);
        for (row, index) in rows(group).enumerate() {
            let part = |top: f32| (Visibility::Hidden, Part(what), Row(top));
            // Each is set in the middle of its row's height, under the first row.
            let within = |height: f32| within(1 + row, height);
            let box_right = MARGIN + TRACK.0 + GAP;
            commands.spawn((text(tuned(index).name), placed(within(NAME * 1.2), box_right + BOX.0 + GAP, Val::Auto, NAME * 1.2), ZIndex(2), part(within(NAME * 1.2))));
            commands.spawn((placed(within(BOX.1), box_right, Val::Px(BOX.0), BOX.1), BackgroundColor(Color::srgb(0.16, 0.16, 0.16)), ZIndex(1), part(within(BOX.1))));
            commands.spawn((text(""), placed(within(NAME * 1.2), box_right + 6.0, Val::Auto, NAME * 1.2), Valued(index), ZIndex(2), part(within(NAME * 1.2))));
            commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(TRACK.0), TRACK.1), BackgroundColor(Color::srgb(0.3, 0.3, 0.3)), ZIndex(1), part(within(TRACK.1))));
            commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(0.0), TRACK.1), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), Moved { index, knob: false }, ZIndex(2), part(within(TRACK.1))));
            commands.spawn((placed(within(KNOB.1), MARGIN, Val::Px(KNOB.0), KNOB.1), BackgroundColor(Color::WHITE), Moved { index, knob: true }, ZIndex(3), part(within(KNOB.1))));
        }
    }
    let tip = Node { position_type: PositionType::Absolute, right: Val::Px(MARGIN + PANEL + 8.0), max_width: Val::Px(TIP), padding: UiRect::all(Val::Px(8.0)), ..default() };
    commands.spawn((text(""), tip, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)), ZIndex(4), Visibility::Hidden, Tip));
}

/// Opens the menu by `U`, the sliders closed; closes whatever is open.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, hands: Res<Hands>) {
    if keys.just_pressed(KeyCode::KeyU) && hands.typed.is_none() {
        show(if shown() == Shown::Closed { Shown::Menu } else { Shown::Closed });
    }
}

/// The digit or the point a key types, if it types one.
fn typed(key: KeyCode) -> Option<char> {
    const DIGITS: [(KeyCode, KeyCode); 10] = [
        (KeyCode::Digit0, KeyCode::Numpad0),
        (KeyCode::Digit1, KeyCode::Numpad1),
        (KeyCode::Digit2, KeyCode::Numpad2),
        (KeyCode::Digit3, KeyCode::Numpad3),
        (KeyCode::Digit4, KeyCode::Numpad4),
        (KeyCode::Digit5, KeyCode::Numpad5),
        (KeyCode::Digit6, KeyCode::Numpad6),
        (KeyCode::Digit7, KeyCode::Numpad7),
        (KeyCode::Digit8, KeyCode::Numpad8),
        (KeyCode::Digit9, KeyCode::Numpad9),
    ];
    if matches!(key, KeyCode::Period | KeyCode::NumpadDecimal | KeyCode::Comma | KeyCode::NumpadComma) {
        return Some('.');
    }
    DIGITS.iter().position(|&(digit, numpad)| key == digit || key == numpad).map(|digit| (b'0' + digit as u8) as char)
}

/// Works what is shown: the menu opened, a group opened or left, the
/// lab's button pressed; and in a group a slider dragged with the left
/// button or set back with the right, a value typed into its box; the
/// numbers kept when one is settled; and each shown as it is.
pub fn slide(
    mut hands: ResMut<Hands>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    mut moved: Query<(&Moved, &mut Node)>,
    mut values: Query<(&Valued, &mut Text)>,
) {
    let pressed = buttons.just_pressed(MouseButton::Left);
    let Some(page) = page() else {
        // The button, or the menu: a row clicked is all there is to them.
        HELD.store(buttons.pressed(MouseButton::Left) && (held() || (pressed && over(&window))), Ordering::Relaxed);
        *hands = Hands::default();
        let groups: Vec<Group> = listed().collect();
        match (shown(), row_under(&window).filter(|_| pressed)) {
            (Shown::Closed, Some(_)) => show(Shown::Menu),
            (Shown::Menu, Some(0)) => show(Shown::Closed),
            (Shown::Menu, Some(row)) => match groups.get(row - 1) {
                Some(&group) => show(Shown::Group(group)),
                None => reseed(),
            },
            _ => {}
        }
        return;
    };
    // A group's first row goes back to the menu.
    if pressed && row_under(&window) == Some(0) && hands.typed.is_none() {
        show(Shown::Menu);
        HELD.store(true, Ordering::Relaxed);
        *hands = Hands::default();
        return;
    }
    let track_left = window.width() - MARGIN - TRACK.0;
    let box_left = track_left - GAP - BOX.0;
    let pointer = window.cursor_position();
    // The number whose row the pointer is on: under the first row.
    let on_row = row_under(&window).and_then(|row| rows(page).nth(row.checked_sub(1)?));
    let on_track = on_row.filter(|_| pointer.is_some_and(|pointer| pointer.x >= track_left - KNOB.0));
    let on_box = on_row.filter(|_| pointer.is_some_and(|pointer| (box_left..box_left + BOX.0).contains(&pointer.x)));

    // What is being typed is set by Enter or a click anywhere, and left by Escape.
    let settles = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) || buttons.just_pressed(MouseButton::Left);
    if let Some((index, digits)) = &mut hands.typed {
        keys.get_just_pressed().filter_map(|&key| typed(key)).for_each(|character| digits.push(character));
        if keys.just_pressed(KeyCode::Backspace) {
            digits.pop();
        }
        if settles {
            if let Ok(value) = digits.parse() {
                set(*index, value);
                keep();
            }
        }
        if settles || keys.just_pressed(KeyCode::Escape) {
            hands.typed = None;
        }
    }
    if buttons.just_pressed(MouseButton::Left) {
        HELD.store(over(&window), Ordering::Relaxed);
        hands.dragged = on_track;
        hands.typed = on_box.map(|index| (index, String::new()));
    }
    if !buttons.pressed(MouseButton::Left) {
        HELD.store(false, Ordering::Relaxed);
    }
    if let (Some(index), Some(pointer)) = (hands.dragged, pointer) {
        let (least, most) = tuned(index).range;
        set(index, least + (most - least) * ((pointer.x - track_left) / TRACK.0).clamp(0.0, 1.0));
    }
    if let Some(index) = on_track.filter(|_| buttons.just_pressed(MouseButton::Right)) {
        set(index, unless_set(index));
        keep();
    }
    if buttons.just_released(MouseButton::Left) && hands.dragged.take().is_some() {
        keep();
    }

    let numbers = now();
    for (part, mut node) in &mut moved {
        let (least, most) = tuned(part.index).range;
        // A value typed past the slider's range leaves the knob at its end.
        let filled = TRACK.0 * ((numbers[part.index] - least) / (most - least)).clamp(0.0, 1.0);
        // The filled part from the track's left, the knob's middle where it ends.
        let (width, right) = if part.knob { (KNOB.0, MARGIN + TRACK.0 - filled - KNOB.0 / 2.0) } else { (filled, MARGIN + TRACK.0 - filled) };
        if node.right != Val::Px(right) {
            (node.width, node.right) = (Val::Px(width), Val::Px(right));
        }
    }
    for (valued, mut text) in &mut values {
        let value = match &hands.typed {
            Some((index, digits)) if *index == valued.0 => format!("{digits}_"),
            _ => format!("{:.2}", numbers[valued.0]),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Says what a slider does, beside its row, once the pointer has
/// rested on it a moment -- and no more once it moves.
pub fn tell(mut hands: ResMut<Hands>, time: Res<Time>, window: Single<&Window>, tip: Single<(&mut Text, &mut Node, &mut Visibility), With<Tip>>) {
    let (mut said, mut node, mut shown) = tip.into_inner();
    let pointer = window.cursor_position();
    let rested = match pointer {
        Some(pointer) if pointer == hands.rested.0 => hands.rested.1 + time.delta_secs(),
        _ => 0.0,
    };
    hands.rested = (pointer.unwrap_or(Vec2::NEG_ONE), rested);
    // The row the pointer is on, of the group shown, and its number: under the first row.
    let told = page().zip(row_under(&window)).and_then(|(page, row)| Some((row, rows(page).nth(row.checked_sub(1)?)?))).filter(|_| rested >= REST && hands.dragged.is_none());
    match told {
        Some((row, index)) => {
            if said.0 != tuned(index).what {
                said.0 = tuned(index).what.to_string();
            }
            node.top = Val::Px(MARGIN + ROW * row as f32 - scrolled());
            shown.set_if_neq(Visibility::Visible);
        }
        None => {
            shown.set_if_neq(Visibility::Hidden);
        }
    }
}
