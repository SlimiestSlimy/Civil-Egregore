//! Sliders over the window's top right corner, one a number of
//! [`crate::tuning`], a page of them at a time ([`Page`]): its knob
//! dragged with the left button, set back to its default with the
//! right, and kept when let go. Beside each, a box with its value: a
//! click on it, and the value is typed -- digits and a point, Enter to
//! set it, Escape to leave it. Under generation's page, a button that
//! draws the world's seed again.
//!
//! `U` goes from one page to the next, and to none.
//!
//! They are laid out by plain arithmetic -- a row each, a track of a
//! fixed width against the window's right edge -- so where the pointer
//! is on one is worked out from the same numbers, with no asking Bevy.

use crate::lab::reseed;
use crate::tuning::{keep, now, set, Page, TUNED};
use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

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
/// Screen pixels the button is high.
const BUTTON: f32 = 32.0;

/// The pages, in the order `U` goes through them: the first the lab
/// starts on.
const PAGES: [Page; 2] = [Page::Generation, Page::Shading];

/// Whether the left button went down over the sliders and is still
/// held: the view is then not dragged, wherever the pointer goes.
static HELD: AtomicBool = AtomicBool::new(false);

/// The page shown: its place in [`PAGES`], or their number for none.
static SHOWN: AtomicU8 = AtomicU8::new(PAGES.len() as u8);

/// Whether generation's page is one of those `U` goes through: only in the lab.
static IN_LAB: AtomicBool = AtomicBool::new(false);

/// Anything of the sliders, and the page it is on: shown and hidden with it.
#[derive(Component)]
pub struct Part(Page);

/// A part of a slider that moves with its number.
#[derive(Component)]
pub struct Moved {
    /// The number's place in [`TUNED`].
    index: usize,
    /// Whether it is the knob, or the filled part of the track.
    knob: bool,
}

/// The value of the `.0`-th number, in its box.
#[derive(Component)]
pub struct Valued(usize);

/// What the pointer and the keys are doing with the sliders.
#[derive(Resource, Default)]
pub struct Hands {
    /// The slider being dragged, if one is.
    dragged: Option<usize>,
    /// The number whose value is being typed, and what is typed so far.
    typed: Option<(usize, String)>,
}

/// The page shown, if one is.
fn page() -> Option<Page> {
    PAGES.get(SHOWN.load(Ordering::Relaxed) as usize).copied()
}

/// The numbers on `page`, by their places in [`TUNED`], a row each.
fn rows(page: Page) -> impl Iterator<Item = usize> {
    (0..TUNED.len()).filter(move |&index| TUNED[index].page == page)
}

/// The lab's sliders: generation's page, shown from the start.
pub fn show_generation() {
    IN_LAB.store(true, Ordering::Relaxed);
    SHOWN.store(0, Ordering::Relaxed);
}

/// Whether the sliders have the left button: it went down over them.
pub fn held() -> bool {
    HELD.load(Ordering::Relaxed)
}

/// The top of the button, under `page`'s rows.
fn button_top(page: Page) -> f32 {
    MARGIN + ROW * rows(page).count() as f32 + 4.0
}

/// Whether the pointer is over the page shown.
fn pointer_over(window: &Window) -> bool {
    let Some((page, pointer)) = page().zip(window.cursor_position()) else {
        return false;
    };
    pointer.x >= window.width() - MARGIN - PANEL && pointer.y <= button_top(page) + BUTTON + MARGIN
}

/// The sliders, a page over one dark panel: a name, a box with the
/// value, a track, its filled part and a knob each; and generation's
/// button.
pub fn setup(mut commands: Commands) {
    let placed = |top: f32, right: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top), right: Val::Px(right), width, height: Val::Px(height), ..default() };
    let text = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME), ..default() });
    // Bevy stacks them by `ZIndex`, not by the order they are made in: the panel under all, the knobs over all.
    for shown in PAGES {
        let visibility = if page() == Some(shown) { Visibility::Visible } else { Visibility::Hidden };
        let part = || (visibility, Part(shown));
        let button = if shown == Page::Generation { BUTTON + 12.0 } else { 0.0 };
        commands.spawn((placed(MARGIN - 8.0, MARGIN - 12.0, Val::Px(PANEL + 12.0), ROW * rows(shown).count() as f32 + 8.0 + button), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), ZIndex(0), part()));
        for (row, index) in rows(shown).enumerate() {
            // Each is set in the middle of its row's height.
            let middle = MARGIN + ROW * (row as f32 + 0.5) - 4.0;
            let within = |height: f32| middle - height / 2.0;
            let box_right = MARGIN + TRACK.0 + GAP;
            commands.spawn((text(TUNED[index].name), placed(within(NAME * 1.2), box_right + BOX.0 + GAP, Val::Auto, NAME * 1.2), ZIndex(2), part()));
            commands.spawn((placed(within(BOX.1), box_right, Val::Px(BOX.0), BOX.1), BackgroundColor(Color::srgb(0.16, 0.16, 0.16)), ZIndex(1), part()));
            commands.spawn((text(""), placed(within(NAME * 1.2), box_right + 6.0, Val::Auto, NAME * 1.2), Valued(index), ZIndex(2), part()));
            commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(TRACK.0), TRACK.1), BackgroundColor(Color::srgb(0.3, 0.3, 0.3)), ZIndex(1), part()));
            commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(0.0), TRACK.1), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), Moved { index, knob: false }, ZIndex(2), part()));
            commands.spawn((placed(within(KNOB.1), MARGIN, Val::Px(KNOB.0), KNOB.1), BackgroundColor(Color::WHITE), Moved { index, knob: true }, ZIndex(3), part()));
        }
        if shown == Page::Generation {
            commands.spawn((placed(button_top(shown), MARGIN, Val::Px(TRACK.0), BUTTON), BackgroundColor(Color::srgb(0.2, 0.35, 0.6)), ZIndex(1), part()));
            commands.spawn((text("regenerate, a random seed"), placed(button_top(shown) + 5.0, MARGIN + 12.0, Val::Auto, NAME * 1.2), ZIndex(2), part()));
        }
    }
}

/// Goes to the next page, or to none after the last, by `U`.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut parts: Query<(&Part, &mut Visibility)>) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    // Round the pages and none; generation's is passed over outside the lab.
    let mut next = (SHOWN.load(Ordering::Relaxed) as usize + 1) % (PAGES.len() + 1);
    if PAGES.get(next) == Some(&Page::Generation) && !IN_LAB.load(Ordering::Relaxed) {
        next += 1;
    }
    SHOWN.store(next as u8, Ordering::Relaxed);
    for (part, mut visibility) in &mut parts {
        *visibility = if Some(part.0) == page() { Visibility::Visible } else { Visibility::Hidden };
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

/// Works the page shown: a slider dragged with the left button or set
/// back with the right, a value typed into its box, the button
/// pressed; the numbers kept when one is settled; and each shown as it
/// is.
pub fn slide(
    mut hands: ResMut<Hands>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    mut moved: Query<(&Moved, &mut Node)>,
    mut values: Query<(&Valued, &mut Text)>,
) {
    let Some(page) = page() else {
        HELD.store(false, Ordering::Relaxed);
        *hands = Hands::default();
        return;
    };
    let track_left = window.width() - MARGIN - TRACK.0;
    let box_left = track_left - GAP - BOX.0;
    let pointer = window.cursor_position();
    // The number whose row the pointer is on.
    let on_row = pointer.filter(|pointer| pointer.y >= MARGIN).and_then(|pointer| rows(page).nth(((pointer.y - MARGIN) / ROW) as usize));
    let on_track = on_row.filter(|_| pointer.is_some_and(|pointer| pointer.x >= track_left - KNOB.0));
    let on_box = on_row.filter(|_| pointer.is_some_and(|pointer| (box_left..box_left + BOX.0).contains(&pointer.x)));
    let on_button = page == Page::Generation && pointer.is_some_and(|pointer| pointer.x >= track_left && (button_top(page)..button_top(page) + BUTTON).contains(&pointer.y));

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
        HELD.store(pointer_over(&window), Ordering::Relaxed);
        hands.dragged = on_track;
        hands.typed = on_box.map(|index| (index, String::new()));
        if on_button {
            reseed();
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        HELD.store(false, Ordering::Relaxed);
    }
    if let (Some(index), Some(pointer)) = (hands.dragged, pointer) {
        let (least, most) = TUNED[index].range;
        set(index, least + (most - least) * ((pointer.x - track_left) / TRACK.0).clamp(0.0, 1.0));
    }
    if let Some(index) = on_track.filter(|_| buttons.just_pressed(MouseButton::Right)) {
        set(index, TUNED[index].default);
        keep();
    }
    if buttons.just_released(MouseButton::Left) && hands.dragged.take().is_some() {
        keep();
    }

    let numbers = now();
    for (part, mut node) in &mut moved {
        let (least, most) = TUNED[part.index].range;
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
