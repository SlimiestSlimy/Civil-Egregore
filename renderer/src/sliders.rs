//! Sliders over the window's top right corner, one a number of
//! [`crate::tuning`]: dragged with the left button, set back to its
//! default with the right, and kept when let go.
//!
//! They are laid out by plain arithmetic -- a row each, a track of a
//! fixed width against the window's right edge -- so where the pointer
//! is on one is worked out from the same numbers, with no asking Bevy.

use crate::tuning::{keep, now, set, TUNED};
use bevy::prelude::*;

/// Screen pixels from the window's top and right edges to the sliders.
const MARGIN: f32 = 10.0;
/// Screen pixels from one slider to the next.
const ROW: f32 = 24.0;
/// Screen pixels a slider's track is across, and high.
const TRACK: (f32, f32) = (200.0, 16.0);
/// Screen pixels the sliders and their names take across.
const PANEL: f32 = TRACK.0 + 190.0;

/// The filled part of the `.0`-th slider's track.
#[derive(Component)]
pub struct Fill(usize);

/// The `.0`-th slider's name and value.
#[derive(Component)]
pub struct Named(usize);

/// The slider being dragged, if one is.
#[derive(Resource, Default)]
pub struct Dragged(Option<usize>);

/// Whether the pointer is over the sliders: the view is not to be
/// dragged from there.
pub fn pointer_over(window: &Window) -> bool {
    window.cursor_position().is_some_and(|pointer| pointer.x >= window.width() - MARGIN - PANEL && pointer.y <= MARGIN + ROW * TUNED.len() as f32)
}

/// The sliders: a name, a track and its filled part each.
pub fn setup(mut commands: Commands) {
    for index in 0..TUNED.len() {
        let top = Val::Px(MARGIN + ROW * index as f32);
        let placed = |right: f32, width: Val| Node { position_type: PositionType::Absolute, top, right: Val::Px(right), width, height: Val::Px(TRACK.1), ..default() };
        commands.spawn((placed(MARGIN - 4.0, Val::Px(PANEL)), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6))));
        commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(14.0), ..default() }, placed(MARGIN + TRACK.0 + 8.0, Val::Auto), Named(index)));
        commands.spawn((placed(MARGIN, Val::Px(TRACK.0)), BackgroundColor(Color::srgb(0.25, 0.25, 0.25))));
        commands.spawn((placed(MARGIN, Val::Px(0.0)), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), Fill(index)));
    }
}

/// Moves the slider under the pointer with the left button, sets it
/// back with the right, keeps the numbers when it is let go, and shows
/// each as it is.
pub fn slide(
    mut dragged: ResMut<Dragged>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut names: Query<(&Named, &mut Text)>,
    mut shown: Local<bool>,
) {
    let track_left = window.width() - MARGIN - TRACK.0;
    let pointer = window.cursor_position();
    // The slider whose row the pointer is on, if it is on its track.
    let under = pointer.filter(|pointer| pointer.x >= track_left - 6.0 && pointer.y >= MARGIN).map(|pointer| ((pointer.y - MARGIN) / ROW) as usize).filter(|&index| index < TUNED.len());
    if buttons.just_pressed(MouseButton::Left) {
        dragged.0 = under;
    }
    let mut changed = !*shown;
    if let (Some(index), Some(pointer)) = (dragged.0, pointer) {
        let (least, most) = TUNED[index].range;
        set(index, least + (most - least) * ((pointer.x - track_left) / TRACK.0).clamp(0.0, 1.0));
        changed = true;
    }
    if let Some(index) = under.filter(|_| buttons.just_pressed(MouseButton::Right)) {
        set(index, TUNED[index].default);
        keep();
        changed = true;
    }
    if buttons.just_released(MouseButton::Left) && dragged.0.take().is_some() {
        keep();
    }
    if !changed {
        return;
    }
    *shown = true;
    let values = now();
    for (fill, mut node) in &mut fills {
        let (least, most) = TUNED[fill.0].range;
        let filled = TRACK.0 * (values[fill.0] - least) / (most - least);
        // Filled from the track's left: its right edge that far short of the track's.
        (node.width, node.right) = (Val::Px(filled), Val::Px(MARGIN + TRACK.0 - filled));
    }
    for (named, mut text) in &mut names {
        text.0 = format!("{} {:.2}", TUNED[named.0].name, values[named.0]);
    }
}
