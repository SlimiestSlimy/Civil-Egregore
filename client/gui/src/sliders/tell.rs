//! What a slider does, said beside it once the pointer has rested on
//! its row a moment (`Tuned::what_it_does`).

use super::spawn::SliderTip;
use super::{group_rows, Sliders, SLIDERS_MARGIN_PIXELS, ROW_STEP_PIXELS};
use bevy::prelude::*;
use utilities::tuning::tuned_number;

/// Seconds the pointer rests on a row before what its slider does is said.
const REST_SECONDS: f32 = 0.4;

/// Says what a slider does, beside its row, once the pointer has
/// rested on it a moment -- and no more once it moves.
pub fn tell_what_slider_does(mut sliders: ResMut<Sliders>, frame_time: Res<Time>, window: Single<&Window>, tip_parts: Single<(&mut Text, &mut Node, &mut Visibility), With<SliderTip>>) {
    let (mut tip_text, mut node, mut tip_visibility) = tip_parts.into_inner();
    let pointer_position = window.cursor_position();
    let rested_seconds = match pointer_position {
        Some(still_position) if still_position == sliders.pointer_rest.0 => sliders.pointer_rest.1 + frame_time.delta_secs(),
        _ => 0.0,
    };
    sliders.pointer_rest = (pointer_position.unwrap_or(Vec2::NEG_ONE), rested_seconds);
    // The row the pointer is on, of the group shown, and its number: under the first row.
    let told_slider = sliders.shown_group().zip(sliders.row_under(&window)).and_then(|(shown_group, pointed_row)| Some((pointed_row, group_rows(shown_group).nth(pointed_row.checked_sub(1)?)?)));
    match told_slider.filter(|_| rested_seconds >= REST_SECONDS && sliders.dragged_slider.is_none()) {
        Some((row_number, slider_index)) => {
            if tip_text.0 != tuned_number(slider_index).what_it_does {
                tip_text.0 = tuned_number(slider_index).what_it_does.to_string();
            }
            node.top = Val::Px(SLIDERS_MARGIN_PIXELS + ROW_STEP_PIXELS * row_number as f32 - sliders.scrolled_pixels);
            tip_visibility.set_if_neq(Visibility::Visible);
        }
        None => _ = tip_visibility.set_if_neq(Visibility::Hidden),
    }
}
