//! The sliders' parts, made once, hidden: shown and hidden after as
//! they are offered and opened (`super::scroll`).

use super::{row_middle, group_rows, Part, PartTop, SlidersShown, VALUE_BOX_SIZE_PIXELS, CLOSED_BUTTON_WIDTH_PIXELS, GAP, KNOB_SIZE_PIXELS, SLIDERS_MARGIN_PIXELS, PANEL_WIDTH_PIXELS, ROW_STEP_PIXELS, TRACK_SIZE_PIXELS};
use bevy::prelude::*;
use utilities::tuning::{tuned_number, SliderGroup, SLIDER_GROUPS};

/// The height of a slider's name and value, in screen pixels.
pub(super) const NAME_HEIGHT_PIXELS: f32 = 18.0;
/// Screen pixels across what a slider does is said in, at most.
const TIP_WIDTH_PIXELS: f32 = 380.0;

/// The value of the `.0`-th number, in its box.
#[derive(Component)]
pub struct ValueInBox(pub(super) usize);

/// A part of a slider that moves with its number.
#[derive(Component)]
pub struct MovesWithNumber {
    /// The number's place in `NAMES`.
    pub(super) slider_index: usize,
    /// Whether it is the knob, or the filled part of the track.
    pub(super) is_knob: bool,
}

/// Where what a slider does is said.
#[derive(Component)]
pub struct SliderTip;

/// Over the menus Escape opens and the main menu: a part's stacking,
/// `stack_above` over the sliders' lowest.
fn over_menus(stack_above: i32) -> GlobalZIndex {
    GlobalZIndex(20 + stack_above)
}

/// The button, the menu -- made twice, for every group and for those
/// offered over a world running -- and every group
/// (`docs/gui.md`, "Sliders").
pub fn spawn_sliders(mut bevy_commands: Commands) {
    debug_assert!(SLIDER_GROUPS.is_sorted_by_key(|listed_group| listed_group.setup_only()), "the groups only of setting a world up are listed last");
    let placed_node = |top_pixels: f32, right_pixels: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top_pixels), right: Val::Px(right_pixels), width, height: Val::Px(height), ..default() };
    let text_parts = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME_HEIGHT_PIXELS), ..default() });
    let centred_top = |centred_row: usize, height: f32| row_middle(centred_row) - height / 2.0;
    let part = |part_shown_with: SlidersShown, part_every_group_only: Option<bool>, part_top: f32| (Visibility::Hidden, Part { shown_with: part_shown_with, every_group_only: part_every_group_only }, PartTop(part_top));
    // Stacked by their z, not by the order they are made in: the panel under all, the knobs over all.
    let spawn_panel = |panel_commands: &mut Commands, panel_shown_with: SlidersShown, panel_every_group_only: Option<bool>, panel_width: f32, panel_rows: usize| {
        panel_commands.spawn((placed_node(SLIDERS_MARGIN_PIXELS - 8.0, SLIDERS_MARGIN_PIXELS - 12.0, Val::Px(panel_width + 12.0), ROW_STEP_PIXELS * panel_rows as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), over_menus(0), part(panel_shown_with, panel_every_group_only, SLIDERS_MARGIN_PIXELS - 8.0)));
    };
    // A row that is clicked: a bar, and what the click does.
    let bar = |bar_commands: &mut Commands, bar_shown_with: SlidersShown, bar_every_group_only: Option<bool>, bar_row: usize, bar_width: f32, words: &str, bar_colour: Color| {
        bar_commands.spawn((placed_node(centred_top(bar_row, ROW_STEP_PIXELS - 8.0), SLIDERS_MARGIN_PIXELS, Val::Px(bar_width - 12.0), ROW_STEP_PIXELS - 8.0), BackgroundColor(bar_colour), over_menus(1), part(bar_shown_with, bar_every_group_only, centred_top(bar_row, ROW_STEP_PIXELS - 8.0))));
        bar_commands.spawn((text_parts(words), placed_node(centred_top(bar_row, NAME_HEIGHT_PIXELS * 1.2), SLIDERS_MARGIN_PIXELS + 10.0, Val::Px(bar_width - 32.0), NAME_HEIGHT_PIXELS * 1.2), over_menus(2), part(bar_shown_with, bar_every_group_only, centred_top(bar_row, NAME_HEIGHT_PIXELS * 1.2))));
    };
    let (head_colour, entry_colour) = (Color::srgb(0.22, 0.22, 0.22), Color::srgb(0.14, 0.14, 0.14));
    spawn_panel(&mut bevy_commands, SlidersShown::ClosedButton, None, CLOSED_BUTTON_WIDTH_PIXELS, 1);
    bar(&mut bevy_commands, SlidersShown::ClosedButton, None, 0, CLOSED_BUTTON_WIDTH_PIXELS, "sliders", head_colour);
    let running_group_count = SLIDER_GROUPS.into_iter().filter(|counted_group| !counted_group.setup_only()).count();
    for (every_group, group_count) in [(true, SLIDER_GROUPS.len()), (false, running_group_count)] {
        spawn_panel(&mut bevy_commands, SlidersShown::GroupMenu, Some(every_group), PANEL_WIDTH_PIXELS, 1 + group_count);
        bar(&mut bevy_commands, SlidersShown::GroupMenu, Some(every_group), 0, PANEL_WIDTH_PIXELS, "sliders: a group to open, here or U to close", head_colour);
    }
    for (group_row, menu_group) in SLIDER_GROUPS.into_iter().enumerate() {
        bar(&mut bevy_commands, SlidersShown::GroupMenu, menu_group.setup_only().then_some(true), 1 + group_row, PANEL_WIDTH_PIXELS, menu_group.menu_name(), entry_colour);
    }
    for spawned_group in SLIDER_GROUPS {
        spawn_group(&mut bevy_commands, spawned_group);
    }
    let tip_node = Node { position_type: PositionType::Absolute, right: Val::Px(SLIDERS_MARGIN_PIXELS + PANEL_WIDTH_PIXELS + 8.0), max_width: Val::Px(TIP_WIDTH_PIXELS), padding: UiRect::all(Val::Px(8.0)), ..default() };
    bevy_commands.spawn((text_parts(""), tip_node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)), over_menus(4), Visibility::Hidden, SliderTip));
}

/// `tuning_group`'s page: its panel, its first row, and its sliders.
fn spawn_group(bevy_commands: &mut Commands, tuning_group: SliderGroup) {
    let placed_node = |top_pixels: f32, right_pixels: f32, width: Val, height: f32| Node { position_type: PositionType::Absolute, top: Val::Px(top_pixels), right: Val::Px(right_pixels), width, height: Val::Px(height), ..default() };
    let text_parts = |words: &str| (Text::new(words), TextFont { font_size: FontSize::Px(NAME_HEIGHT_PIXELS), ..default() });
    let shown_with = SlidersShown::OneGroup(tuning_group);
    let part = |part_top: f32| (Visibility::Hidden, Part { shown_with, every_group_only: None }, PartTop(part_top));
    let centred_top = |centred_row: usize, height: f32| row_middle(centred_row) - height / 2.0;
    let (panel_width, sliders) = (PANEL_WIDTH_PIXELS, group_rows(tuning_group).count());
    bevy_commands.spawn((placed_node(SLIDERS_MARGIN_PIXELS - 8.0, SLIDERS_MARGIN_PIXELS - 12.0, Val::Px(panel_width + 12.0), ROW_STEP_PIXELS * (1 + sliders) as f32 + 8.0), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), over_menus(0), part(SLIDERS_MARGIN_PIXELS - 8.0)));
    bevy_commands.spawn((placed_node(centred_top(0, ROW_STEP_PIXELS - 8.0), SLIDERS_MARGIN_PIXELS, Val::Px(panel_width - 12.0), ROW_STEP_PIXELS - 8.0), BackgroundColor(Color::srgb(0.22, 0.22, 0.22)), over_menus(1), part(centred_top(0, ROW_STEP_PIXELS - 8.0))));
    bevy_commands.spawn((text_parts(&format!("< groups  |  {}", tuning_group.menu_name())), placed_node(centred_top(0, NAME_HEIGHT_PIXELS * 1.2), SLIDERS_MARGIN_PIXELS + 10.0, Val::Px(panel_width - 32.0), NAME_HEIGHT_PIXELS * 1.2), over_menus(2), part(centred_top(0, NAME_HEIGHT_PIXELS * 1.2))));
    for (slider_row, slider_index) in group_rows(tuning_group).enumerate() {
        // Each is set in the middle of its row's height, under the first row.
        let centred_in_row = |height: f32| centred_top(1 + slider_row, height);
        let box_right = SLIDERS_MARGIN_PIXELS + TRACK_SIZE_PIXELS.0 + GAP;
        bevy_commands.spawn((text_parts(tuned_number(slider_index).setting_name), placed_node(centred_in_row(NAME_HEIGHT_PIXELS * 1.2), box_right + VALUE_BOX_SIZE_PIXELS.0 + GAP, Val::Auto, NAME_HEIGHT_PIXELS * 1.2), over_menus(2), part(centred_in_row(NAME_HEIGHT_PIXELS * 1.2))));
        bevy_commands.spawn((placed_node(centred_in_row(VALUE_BOX_SIZE_PIXELS.1), box_right, Val::Px(VALUE_BOX_SIZE_PIXELS.0), VALUE_BOX_SIZE_PIXELS.1), BackgroundColor(Color::srgb(0.16, 0.16, 0.16)), over_menus(1), part(centred_in_row(VALUE_BOX_SIZE_PIXELS.1))));
        bevy_commands.spawn((text_parts(""), placed_node(centred_in_row(NAME_HEIGHT_PIXELS * 1.2), box_right + 6.0, Val::Auto, NAME_HEIGHT_PIXELS * 1.2), ValueInBox(slider_index), over_menus(2), part(centred_in_row(NAME_HEIGHT_PIXELS * 1.2))));
        bevy_commands.spawn((placed_node(centred_in_row(TRACK_SIZE_PIXELS.1), SLIDERS_MARGIN_PIXELS, Val::Px(TRACK_SIZE_PIXELS.0), TRACK_SIZE_PIXELS.1), BackgroundColor(Color::srgb(0.3, 0.3, 0.3)), over_menus(1), part(centred_in_row(TRACK_SIZE_PIXELS.1))));
        bevy_commands.spawn((placed_node(centred_in_row(TRACK_SIZE_PIXELS.1), SLIDERS_MARGIN_PIXELS, Val::Px(0.0), TRACK_SIZE_PIXELS.1), BackgroundColor(Color::srgb(0.95, 0.8, 0.25)), MovesWithNumber { slider_index, is_knob: false }, over_menus(2), part(centred_in_row(TRACK_SIZE_PIXELS.1))));
        bevy_commands.spawn((placed_node(centred_in_row(KNOB_SIZE_PIXELS.1), SLIDERS_MARGIN_PIXELS, Val::Px(KNOB_SIZE_PIXELS.0), KNOB_SIZE_PIXELS.1), BackgroundColor(Color::WHITE), MovesWithNumber { slider_index, is_knob: true }, over_menus(3), part(centred_in_row(KNOB_SIZE_PIXELS.1))));
    }
}
