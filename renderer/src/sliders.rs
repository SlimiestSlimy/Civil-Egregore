//! Sliders over the window's top right corner, one a number of
//! [`crate::tuning`]: its knob dragged with the left button, set back
//! to its default with the right, and kept when let go.
//!
//! They are laid out by plain arithmetic -- a row each, a track of a
//! fixed width against the window's right edge -- so where the pointer
//! is on one is worked out from the same numbers, with no asking Bevy.

use crate::tuning::{keep, now, set, TUNED};
use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Screen pixels from the window's top and right edges to the sliders.
const MARGIN: f32 = 16.0;
/// Screen pixels from one slider to the next.
const ROW: f32 = 36.0;
/// Screen pixels a slider's track is across, and high.
const TRACK: (f32, f32) = (300.0, 8.0);
/// Screen pixels a slider's knob is across, and high.
const KNOB: (f32, f32) = (14.0, 26.0);
/// Screen pixels the sliders and their names take across.
const PANEL: f32 = TRACK.0 + 230.0;
/// The height of a slider's name, in screen pixels.
const NAME: f32 = 18.0;

/// Whether the left button went down over the sliders and is still
/// held: the view is then not dragged, wherever the pointer goes.
static HELD: AtomicBool = AtomicBool::new(false);

/// The filled part of the `.0`-th slider's track.
#[derive(Component)]
pub struct Fill(usize);

/// The `.0`-th slider's name and value.
#[derive(Component)]
pub struct Named(usize);

/// The slider being dragged, if one is.
#[derive(Resource, Default)]
pub struct Dragged(Option<usize>);

/// The knob of the `.0`-th slider.
#[derive(Component)]
pub struct Knob(usize);

/// Whether the sliders have the left button: it went down over them.
pub fn held() -> bool {
    HELD.load(Ordering::Relaxed)
}

/// Whether the pointer is over the sliders.
fn pointer_over(window: &Window) -> bool {
    window.cursor_position().is_some_and(|pointer| pointer.x >= window.width() - MARGIN - PANEL && pointer.y <= MARGIN + ROW * TUNED.len() as f32)
}

/// The sliders: over one dark panel, a name, a track, its filled part
/// and a knob each.
pub fn setup(mut commands: Commands) {
    let placed = |top: f32, right: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top), right: Val::Px(right), width, height: Val::Px(height), ..default() };
    commands.spawn((placed(MARGIN - 8.0, MARGIN - 12.0, Val::Px(PANEL + 12.0), ROW * TUNED.len() as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))));
    for index in 0..TUNED.len() {
        // Each is set in the middle of its row's height.
        let middle = MARGIN + ROW * (index as f32 + 0.5) - 4.0;
        let within = |height: f32| middle - height / 2.0;
        commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(NAME), ..default() }, placed(within(NAME * 1.2), MARGIN + TRACK.0 + 16.0, Val::Auto, NAME * 1.2), Named(index)));
        commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(TRACK.0), TRACK.1), BackgroundColor(Color::srgb(0.3, 0.3, 0.3))));
        commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(0.0), TRACK.1), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), Fill(index)));
        commands.spawn((placed(within(KNOB.1), MARGIN, Val::Px(KNOB.0), KNOB.1), BackgroundColor(Color::WHITE), Knob(index)));
    }
}

/// Moves the slider under the pointer with the left button, sets it
/// back with the right, keeps the numbers when it is let go, and shows
/// each as it is.
pub fn slide(
    mut dragged: ResMut<Dragged>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    mut fills: Query<(&Fill, &mut Node), Without<Knob>>,
    mut knobs: Query<(&Knob, &mut Node)>,
    mut names: Query<(&Named, &mut Text)>,
    mut shown: Local<bool>,
) {
    let track_left = window.width() - MARGIN - TRACK.0;
    let pointer = window.cursor_position();
    // The slider whose row the pointer is on, if it is on its track.
    let under = pointer.filter(|pointer| pointer.x >= track_left - KNOB.0 && pointer.y >= MARGIN).map(|pointer| ((pointer.y - MARGIN) / ROW) as usize).filter(|&index| index < TUNED.len());
    if buttons.just_pressed(MouseButton::Left) {
        dragged.0 = under;
        HELD.store(pointer_over(&window), Ordering::Relaxed);
    }
    if !buttons.pressed(MouseButton::Left) {
        HELD.store(false, Ordering::Relaxed);
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
    for (knob, mut node) in &mut knobs {
        let (least, most) = TUNED[knob.0].range;
        // Its middle where the filled part ends.
        node.right = Val::Px(MARGIN + TRACK.0 * (most - values[knob.0]) / (most - least) - KNOB.0 / 2.0);
    }
    for (named, mut text) in &mut names {
        text.0 = format!("{} {:.2}", TUNED[named.0].name, values[named.0]);
    }
}
