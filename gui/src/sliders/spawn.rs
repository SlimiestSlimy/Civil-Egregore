//! The sliders' parts, made once, hidden: shown and hidden after as
//! they are offered and opened (`super::scroll`).

use super::{middle, rows, Part, Row, Shown, BOX, CLOSED, GAP, KNOB, MARGIN, PANEL, ROW, TRACK};
use bevy::prelude::*;
use utilities::tuning::{tuned, Group, GROUPS};

/// The height of a slider's name and value, in screen pixels.
pub(super) const NAME: f32 = 18.0;
/// Screen pixels across what a slider does is said in, at most.
const TIP: f32 = 380.0;

/// The value of the `.0`-th number, in its box.
#[derive(Component)]
pub struct Valued(pub(super) usize);

/// A part of a slider that moves with its number.
#[derive(Component)]
pub struct Moved {
    /// The number's place in `NAMES`.
    pub(super) index: usize,
    /// Whether it is the knob, or the filled part of the track.
    pub(super) knob: bool,
}

/// Where what a slider does is said.
#[derive(Component)]
pub struct Tip;

/// Over the menus Escape opens and the main menu: a part's stacking,
/// `z` over the sliders' lowest.
fn over_menus(z: i32) -> GlobalZIndex {
    GlobalZIndex(20 + z)
}

/// The button, the menu -- made twice, for every group and for those
/// offered over a world running -- and every group
/// (`docs/gui.md`, "Sliders").
pub fn spawn(mut commands: Commands) {
    debug_assert!(GROUPS.is_sorted_by_key(|group| group.setup_only()), "the groups only of setting a world up are listed last");
    let placed = |top: f32, right: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top), right: Val::Px(right), width, height: Val::Px(height), ..default() };
    let text = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME), ..default() });
    let within = |row: usize, height: f32| middle(row) - height / 2.0;
    let part = |shown: Shown, everything: Option<bool>, top: f32| (Visibility::Hidden, Part { shown, everything }, Row(top));
    // Stacked by their z, not by the order they are made in: the panel under all, the knobs over all.
    let panel = |commands: &mut Commands, shown: Shown, everything: Option<bool>, across: f32, rows: usize| {
        commands.spawn((placed(MARGIN - 8.0, MARGIN - 12.0, Val::Px(across + 12.0), ROW * rows as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), over_menus(0), part(shown, everything, MARGIN - 8.0)));
    };
    // A row that is clicked: a bar, and what the click does.
    let bar = |commands: &mut Commands, shown: Shown, everything: Option<bool>, row: usize, across: f32, words: &str, colour: Color| {
        commands.spawn((placed(within(row, ROW - 8.0), MARGIN, Val::Px(across - 12.0), ROW - 8.0), BackgroundColor(colour), over_menus(1), part(shown, everything, within(row, ROW - 8.0))));
        commands.spawn((text(words), placed(within(row, NAME * 1.2), MARGIN + 10.0, Val::Px(across - 32.0), NAME * 1.2), over_menus(2), part(shown, everything, within(row, NAME * 1.2))));
    };
    let (head, entry) = (Color::srgb(0.22, 0.22, 0.22), Color::srgb(0.14, 0.14, 0.14));
    panel(&mut commands, Shown::Closed, None, CLOSED, 1);
    bar(&mut commands, Shown::Closed, None, 0, CLOSED, "sliders", head);
    let running = GROUPS.into_iter().filter(|group| !group.setup_only()).count();
    for (everything, groups) in [(true, GROUPS.len()), (false, running)] {
        panel(&mut commands, Shown::Menu, Some(everything), PANEL, 1 + groups);
        bar(&mut commands, Shown::Menu, Some(everything), 0, PANEL, "sliders: a group to open, here or U to close", head);
    }
    for (row, group) in GROUPS.into_iter().enumerate() {
        bar(&mut commands, Shown::Menu, group.setup_only().then_some(true), 1 + row, PANEL, group.name(), entry);
    }
    for group in GROUPS {
        spawn_group(&mut commands, group);
    }
    let tip = Node { position_type: PositionType::Absolute, right: Val::Px(MARGIN + PANEL + 8.0), max_width: Val::Px(TIP), padding: UiRect::all(Val::Px(8.0)), ..default() };
    commands.spawn((text(""), tip, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)), over_menus(4), Visibility::Hidden, Tip));
}

/// `group`'s page: its panel, its first row, and its sliders.
fn spawn_group(commands: &mut Commands, group: Group) {
    let placed = |top: f32, right: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top), right: Val::Px(right), width, height: Val::Px(height), ..default() };
    let text = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME), ..default() });
    let shown = Shown::Group(group);
    let part = |top: f32| (Visibility::Hidden, Part { shown, everything: None }, Row(top));
    let within = |row: usize, height: f32| middle(row) - height / 2.0;
    let (across, sliders) = (PANEL, rows(group).count());
    commands.spawn((placed(MARGIN - 8.0, MARGIN - 12.0, Val::Px(across + 12.0), ROW * (1 + sliders) as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), over_menus(0), part(MARGIN - 8.0)));
    commands.spawn((placed(within(0, ROW - 8.0), MARGIN, Val::Px(across - 12.0), ROW - 8.0), BackgroundColor(Color::srgb(0.22, 0.22, 0.22)), over_menus(1), part(within(0, ROW - 8.0))));
    commands.spawn((text(&format!("< groups  |  {}", group.name())), placed(within(0, NAME * 1.2), MARGIN + 10.0, Val::Px(across - 32.0), NAME * 1.2), over_menus(2), part(within(0, NAME * 1.2))));
    for (row, index) in rows(group).enumerate() {
        // Each is set in the middle of its row's height, under the first row.
        let within = |height: f32| within(1 + row, height);
        let box_right = MARGIN + TRACK.0 + GAP;
        commands.spawn((text(tuned(index).name), placed(within(NAME * 1.2), box_right + BOX.0 + GAP, Val::Auto, NAME * 1.2), over_menus(2), part(within(NAME * 1.2))));
        commands.spawn((placed(within(BOX.1), box_right, Val::Px(BOX.0), BOX.1), BackgroundColor(Color::srgb(0.16, 0.16, 0.16)), over_menus(1), part(within(BOX.1))));
        commands.spawn((text(""), placed(within(NAME * 1.2), box_right + 6.0, Val::Auto, NAME * 1.2), Valued(index), over_menus(2), part(within(NAME * 1.2))));
        commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(TRACK.0), TRACK.1), BackgroundColor(Color::srgb(0.3, 0.3, 0.3)), over_menus(1), part(within(TRACK.1))));
        commands.spawn((placed(within(TRACK.1), MARGIN, Val::Px(0.0), TRACK.1), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), Moved { index, knob: false }, over_menus(2), part(within(TRACK.1))));
        commands.spawn((placed(within(KNOB.1), MARGIN, Val::Px(KNOB.0), KNOB.1), BackgroundColor(Color::WHITE), Moved { index, knob: true }, over_menus(3), part(within(KNOB.1))));
    }
}
