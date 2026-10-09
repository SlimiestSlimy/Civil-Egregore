//! What a slider does, said beside it once the pointer has rested on
//! its row a moment (`Tuned::what`).

use super::spawn::Tip;
use super::{rows, Sliders, MARGIN, ROW};
use bevy::prelude::*;
use utilities::tuning::tuned;

/// Seconds the pointer rests on a row before what its slider does is said.
const REST: f32 = 0.4;

/// Says what a slider does, beside its row, once the pointer has
/// rested on it a moment -- and no more once it moves.
pub fn tell(mut sliders: ResMut<Sliders>, time: Res<Time>, window: Single<&Window>, tip: Single<(&mut Text, &mut Node, &mut Visibility), With<Tip>>) {
    let (mut said, mut node, mut shown) = tip.into_inner();
    let pointer = window.cursor_position();
    let rested = match pointer {
        Some(pointer) if pointer == sliders.rested.0 => sliders.rested.1 + time.delta_secs(),
        _ => 0.0,
    };
    sliders.rested = (pointer.unwrap_or(Vec2::NEG_ONE), rested);
    // The row the pointer is on, of the group shown, and its number: under the first row.
    let told = sliders.page().zip(sliders.row_under(&window)).and_then(|(page, row)| Some((row, rows(page).nth(row.checked_sub(1)?)?)));
    match told.filter(|_| rested >= REST && sliders.dragged.is_none()) {
        Some((row, index)) => {
            if said.0 != tuned(index).what {
                said.0 = tuned(index).what.to_string();
            }
            node.top = Val::Px(MARGIN + ROW * row as f32 - sliders.scrolled);
            shown.set_if_neq(Visibility::Visible);
        }
        None => _ = shown.set_if_neq(Visibility::Hidden),
    }
}
