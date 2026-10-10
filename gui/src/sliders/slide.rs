//! What a click on the sliders does: the menu and groups opened and
//! left, a knob dragged, a value typed, a toggle turned
//! (`docs/gui.md`, "Sliders").

use super::spawn::{Moved, Valued};
use super::{rows, Offered, Shown, Sliders, BOX, GAP, KNOB, MARGIN, TRACK};
use crate::CurrentTuning;
use bevy::prelude::*;
use utilities::tuning::{keep, settled, tuned, unless_set};

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

/// Whether the toggle that is the `index`-th number is on at `value`:
/// nearer its range's far end than its near one.
fn turned_on(index: usize, value: f32) -> bool {
    let (off, on) = tuned(index).range;
    value >= (off + on) / 2.0
}

/// Works what is shown, as the module says, and shows each slider's
/// knob and value as they are.
pub fn slide(mut sliders: ResMut<Sliders>, mut tuning: ResMut<CurrentTuning>, buttons: Res<ButtonInput<MouseButton>>, keys: Res<ButtonInput<KeyCode>>, window: Single<&Window>, moved: Query<(&Moved, &mut Node)>, values: Query<(&Valued, &mut Text)>) {
    if sliders.offered == Offered::Hidden {
        return;
    }
    let pressed = buttons.just_pressed(MouseButton::Left);
    let Some(page) = sliders.page() else {
        // The button, or the menu: a row clicked is all there is to them.
        sliders.held = buttons.pressed(MouseButton::Left) && (sliders.held || (pressed && sliders.over(&window)));
        let groups: Vec<_> = sliders.listed().collect();
        match (sliders.shown, sliders.row_under(&window).filter(|_| pressed)) {
            (Shown::Closed, Some(_)) => sliders.show(Shown::Menu),
            (Shown::Menu, Some(0)) => sliders.show(Shown::Closed),
            (Shown::Menu, Some(row)) => sliders.show(Shown::Group(groups[row - 1])),
            _ => {}
        }
        return;
    };
    // A group's first row goes back to the menu.
    if pressed && sliders.row_under(&window) == Some(0) && !sliders.typing() {
        sliders.show(Shown::Menu);
        (sliders.held, sliders.dragged) = (true, None);
        return;
    }
    let track_left = window.width() - MARGIN - TRACK.0;
    let box_left = track_left - GAP - BOX.0;
    let pointer = window.cursor_position();
    // The number whose row the pointer is on: under the first row.
    let on_row = sliders.row_under(&window).and_then(|row| rows(page).nth(row.checked_sub(1)?));
    let on_track = on_row.filter(|_| pointer.is_some_and(|pointer| pointer.x >= track_left - KNOB.0));
    let on_box = on_row.filter(|_| pointer.is_some_and(|pointer| (box_left..box_left + BOX.0).contains(&pointer.x)));

    // What is being typed is set by Enter or a click anywhere, and left by Escape.
    let settles = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) || pressed;
    if let Some((index, digits)) = &mut sliders.typed {
        keys.get_just_pressed().filter_map(|&key| typed(key)).for_each(|character| digits.push(character));
        if keys.just_pressed(KeyCode::Backspace) {
            digits.pop();
        }
        if settles && let Ok(value) = digits.parse() {
            tuning.0[*index] = settled(*index, value);
            keep(&tuning.0);
        }
        if settles || keys.just_pressed(KeyCode::Escape) {
            sliders.typed = None;
        }
    }
    if pressed {
        sliders.held = sliders.over(&window);
        // A toggle is turned by the click itself, on its track or its box.
        if let Some(index) = on_track.or(on_box).filter(|&index| tuned(index).toggle) {
            let (off, on) = tuned(index).range;
            tuning.0[index] = if turned_on(index, tuning.0[index]) { off } else { on };
            keep(&tuning.0);
        }
        sliders.dragged = on_track.filter(|&index| !tuned(index).toggle);
        sliders.typed = on_box.filter(|&index| !tuned(index).toggle).map(|index| (index, String::new()));
    }
    if !buttons.pressed(MouseButton::Left) {
        sliders.held = false;
    }
    if let (Some(index), Some(pointer)) = (sliders.dragged, pointer) {
        let (least, most) = tuned(index).range;
        let value = least + (most - least) * ((pointer.x - track_left) / TRACK.0).clamp(0.0, 1.0);
        // Set only when it moves: the painter is sent the numbers each time they change.
        if tuning.0[index] != value {
            tuning.0[index] = value;
        }
    }
    if let Some(index) = on_track.filter(|_| buttons.just_pressed(MouseButton::Right)) {
        tuning.0[index] = unless_set(index);
        keep(&tuning.0);
    }
    if buttons.just_released(MouseButton::Left) && sliders.dragged.take().is_some() {
        keep(&tuning.0);
    }
    show(&sliders, &tuning.0, moved, values);
}

/// Each slider's knob and filled track where its number is, and its
/// value in its box -- or what is being typed into it.
fn show(sliders: &Sliders, numbers: &utilities::tuning::Tuning, mut moved: Query<(&Moved, &mut Node)>, mut values: Query<(&Valued, &mut Text)>) {
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
        let value = match &sliders.typed {
            Some((index, digits)) if *index == valued.0 => format!("{digits}_"),
            _ if tuned(valued.0).toggle => if turned_on(valued.0, numbers[valued.0]) { "on" } else { "off" }.to_string(),
            _ => format!("{:.2}", numbers[valued.0]),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}
