//! What a click on the sliders does: the menu and groups opened and
//! left, a knob dragged, a value typed, a toggle turned
//! (`docs/gui.md`, "Sliders").

use super::spawn::{MovesWithNumber, ValueInBox};
use super::{group_rows, SlidersOffered, SlidersShown, Sliders, VALUE_BOX_SIZE_PIXELS, GAP, KNOB_SIZE_PIXELS, SLIDERS_MARGIN_PIXELS, TRACK_SIZE_PIXELS};
use crate::CurrentTuning;
use bevy::prelude::*;
use utilities::tuning::{save_to_settings, value_or_default, tuned_number, unless_set};

/// The digit or the point a key types, if it types one.
fn typed_digit(pressed_key: KeyCode) -> Option<char> {
    const DIGIT_KEYS: [(KeyCode, KeyCode); 10] = [
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
    if matches!(pressed_key, KeyCode::Period | KeyCode::NumpadDecimal | KeyCode::Comma | KeyCode::NumpadComma) {
        return Some('.');
    }
    DIGIT_KEYS.iter().position(|&(digit_key, numpad_key)| pressed_key == digit_key || pressed_key == numpad_key).map(|digit_value| (b'0' + digit_value as u8) as char)
}

/// Whether the toggle that is the `slider_index`-th number is on at `value`:
/// nearer its range's far end than its near one.
fn turned_on(slider_index: usize, value: f32) -> bool {
    let (off_value, on_value) = tuned_number(slider_index).slider_range;
    value >= (off_value + on_value) / 2.0
}

/// Works what is shown, as the module says, and shows each slider's
/// knob and value as they are.
pub fn work_slider_clicks(mut sliders: ResMut<Sliders>, mut tuning: ResMut<CurrentTuning>, mouse_buttons: Res<ButtonInput<MouseButton>>, pressed_keys: Res<ButtonInput<KeyCode>>, window: Single<&Window>, moved_parts: Query<(&MovesWithNumber, &mut Node)>, values: Query<(&ValueInBox, &mut Text)>) {
    if sliders.offered_groups == SlidersOffered::NoneOffered {
        return;
    }
    let left_pressed = mouse_buttons.just_pressed(MouseButton::Left);
    let Some(shown_group) = sliders.shown_group() else {
        // The button, or the menu: a row clicked is all there is to them.
        sliders.button_held = mouse_buttons.pressed(MouseButton::Left) && (sliders.button_held || (left_pressed && sliders.pointer_over(&window)));
        let menu_groups: Vec<_> = sliders.listed_groups().collect();
        match (sliders.what_shown, sliders.row_under(&window).filter(|_| left_pressed)) {
            (SlidersShown::ClosedButton, Some(_)) => sliders.show_from_top(SlidersShown::GroupMenu),
            (SlidersShown::GroupMenu, Some(0)) => sliders.show_from_top(SlidersShown::ClosedButton),
            (SlidersShown::GroupMenu, Some(row_number)) => sliders.show_from_top(SlidersShown::OneGroup(menu_groups[row_number - 1])),
            _ => {}
        }
        return;
    };
    // A group's first row goes back to the menu.
    if left_pressed && sliders.row_under(&window) == Some(0) && !sliders.typing_value() {
        sliders.show_from_top(SlidersShown::GroupMenu);
        (sliders.button_held, sliders.dragged_slider) = (true, None);
        return;
    }
    let track_left = window.width() - SLIDERS_MARGIN_PIXELS - TRACK_SIZE_PIXELS.0;
    let box_left = track_left - GAP - VALUE_BOX_SIZE_PIXELS.0;
    let pointer_position = window.cursor_position();
    // The number whose row the pointer is on: under the first row.
    let on_row = sliders.row_under(&window).and_then(|pointed_row| group_rows(shown_group).nth(pointed_row.checked_sub(1)?));
    let on_track = on_row.filter(|_| pointer_position.is_some_and(|track_pointer| track_pointer.x >= track_left - KNOB_SIZE_PIXELS.0));
    let on_box = on_row.filter(|_| pointer_position.is_some_and(|box_pointer| (box_left..box_left + VALUE_BOX_SIZE_PIXELS.0).contains(&box_pointer.x)));

    // What is being typed is set by Enter or a click anywhere, and left by Escape.
    let value_settles = pressed_keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) || left_pressed;
    if let Some((typed_index, typed_digits)) = &mut sliders.typed_value {
        pressed_keys.get_just_pressed().filter_map(|&just_pressed_key| typed_digit(just_pressed_key)).for_each(|typed_character| typed_digits.push(typed_character));
        if pressed_keys.just_pressed(KeyCode::Backspace) {
            typed_digits.pop();
        }
        if value_settles && let Ok(value) = typed_digits.parse() {
            tuning.0[*typed_index] = value_or_default(*typed_index, value);
            save_to_settings(&tuning.0);
        }
        if value_settles || pressed_keys.just_pressed(KeyCode::Escape) {
            sliders.typed_value = None;
        }
    }
    if left_pressed {
        sliders.button_held = sliders.pointer_over(&window);
        // A toggle is turned by the click itself, on its track or its box.
        if let Some(toggle_index) = on_track.or(on_box).filter(|&clicked_index| tuned_number(clicked_index).toggle) {
            let (off_value, on_value) = tuned_number(toggle_index).slider_range;
            tuning.0[toggle_index] = if turned_on(toggle_index, tuning.0[toggle_index]) { off_value } else { on_value };
            save_to_settings(&tuning.0);
        }
        sliders.dragged_slider = on_track.filter(|&track_index| !tuned_number(track_index).toggle);
        sliders.typed_value = on_box.filter(|&box_index| !tuned_number(box_index).toggle).map(|typed_slider| (typed_slider, String::new()));
    }
    if !mouse_buttons.pressed(MouseButton::Left) {
        sliders.button_held = false;
    }
    if let (Some(dragged_index), Some(dragged_to)) = (sliders.dragged_slider, pointer_position) {
        let (range_least, range_most) = tuned_number(dragged_index).slider_range;
        let value = range_least + (range_most - range_least) * ((dragged_to.x - track_left) / TRACK_SIZE_PIXELS.0).clamp(0.0, 1.0);
        // Set only when it moves: the painter is sent the numbers each time they change.
        if tuning.0[dragged_index] != value {
            tuning.0[dragged_index] = value;
        }
    }
    if let Some(reset_index) = on_track.filter(|_| mouse_buttons.just_pressed(MouseButton::Right)) {
        tuning.0[reset_index] = unless_set(reset_index);
        save_to_settings(&tuning.0);
    }
    if mouse_buttons.just_released(MouseButton::Left) && sliders.dragged_slider.take().is_some() {
        save_to_settings(&tuning.0);
    }
    update_slider_visuals(&sliders, &tuning.0, moved_parts, values);
}

/// Each slider's knob and filled track where its number is, and its
/// value in its box -- or what is being typed into it.
fn update_slider_visuals(sliders: &Sliders, tuning_numbers: &utilities::tuning::Tuning, mut moved_parts: Query<(&MovesWithNumber, &mut Node)>, mut values: Query<(&ValueInBox, &mut Text)>) {
    for (part, mut node) in &mut moved_parts {
        let (range_least, range_most) = tuned_number(part.slider_index).slider_range;
        // A value typed past the slider's range leaves the knob at its end.
        let filled_pixels = TRACK_SIZE_PIXELS.0 * ((tuning_numbers[part.slider_index] - range_least) / (range_most - range_least)).clamp(0.0, 1.0);
        // The filled part from the track's left, the knob's middle where it ends.
        let (width, right_pixels) = if part.is_knob { (KNOB_SIZE_PIXELS.0, SLIDERS_MARGIN_PIXELS + TRACK_SIZE_PIXELS.0 - filled_pixels - KNOB_SIZE_PIXELS.0 / 2.0) } else { (filled_pixels, SLIDERS_MARGIN_PIXELS + TRACK_SIZE_PIXELS.0 - filled_pixels) };
        if node.right != Val::Px(right_pixels) {
            (node.width, node.right) = (Val::Px(width), Val::Px(right_pixels));
        }
    }
    for (value_marker, mut value_text) in &mut values {
        let value = match &sliders.typed_value {
            Some((typed_index, typed_digits)) if *typed_index == value_marker.0 => format!("{typed_digits}_"),
            _ if tuned_number(value_marker.0).toggle => if turned_on(value_marker.0, tuning_numbers[value_marker.0]) { "on" } else { "off" }.to_string(),
            _ => format!("{:.2}", tuning_numbers[value_marker.0]),
        };
        if value_text.0 != value {
            value_text.0 = value;
        }
    }
}
