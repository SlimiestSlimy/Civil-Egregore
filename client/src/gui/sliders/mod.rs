//! Sliders over the window's top right corner, one a number of
//! `utilities::tuning`, in groups, one group on the screen at a time,
//! opened from a menu (`docs/gui.md`, "Sliders").

mod slide;
mod spawn;
mod tell;

pub use slide::work_slider_clicks;
pub use spawn::spawn_sliders;
pub use tell::tell_what_slider_does;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use utilities::tuning::{tuned_number, SliderGroup, SLIDER_GROUPS, NAMES};

/// Screen pixels from the window's top and right edges to the sliders.
const SLIDERS_MARGIN_PIXELS: f32 = 16.0;
/// Screen pixels from one slider to the next.
const ROW_STEP_PIXELS: f32 = 36.0;
/// Screen pixels a slider's track is across, and high.
const TRACK_SIZE_PIXELS: (f32, f32) = (300.0, 8.0);
/// Screen pixels a slider's knob is across, and high.
const KNOB_SIZE_PIXELS: (f32, f32) = (14.0, 26.0);
/// Screen pixels a value's box is across, and high.
const VALUE_BOX_SIZE_PIXELS: (f32, f32) = (84.0, 26.0);
/// Screen pixels between a slider's name, its box and its track.
const GAP: f32 = 14.0;
/// Screen pixels the sliders, their boxes and their names take across.
const PANEL_WIDTH_PIXELS: f32 = TRACK_SIZE_PIXELS.0 + GAP + VALUE_BOX_SIZE_PIXELS.0 + GAP + 170.0;
/// Screen pixels the button shown while the sliders are closed is across.
const CLOSED_BUTTON_WIDTH_PIXELS: f32 = 96.0;
/// Screen pixels a notch of the wheel scrolls a page.
const NOTCH_SCROLL_PIXELS: f32 = 2.0 * ROW_STEP_PIXELS;

/// What of the sliders is on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlidersShown {
    /// Only the button that opens the menu.
    #[default]
    ClosedButton,
    /// The menu: the groups, a row each.
    GroupMenu,
    /// One group's sliders.
    OneGroup(SliderGroup),
}

/// Which of the sliders are offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlidersOffered {
    /// None: none is shown, and none takes the pointer or the keys.
    #[default]
    NoneOffered,
    /// Every group: a world is being set up.
    EveryGroup,
    /// Every group but those only of setting a world up: a world runs.
    RunningGroups,
}

/// The sliders: what of them is shown, and what the pointer and the
/// keys are doing with them.
#[derive(Resource, Default)]
pub struct Sliders {
    /// What is offered.
    offered_groups: SlidersOffered,
    /// What is shown of it.
    what_shown: SlidersShown,
    /// Screen pixels what is shown is scrolled up by.
    scrolled_pixels: f32,
    /// Whether the left button went down over them and is still held:
    /// the view is then not dragged, wherever the pointer goes.
    button_held: bool,
    /// The slider being dragged, if one is.
    dragged_slider: Option<usize>,
    /// The number whose value is being typed, and what is typed so far.
    typed_value: Option<(usize, String)>,
    /// Where the pointer last was, and the seconds it has rested there.
    pointer_rest: (Vec2, f32),
}

impl Sliders {
    /// Whether they have the left button: it went down over them.
    pub fn holds_left_button(&self) -> bool {
        self.button_held
    }

    /// Whether a value is being typed: the keys are theirs.
    pub fn typing_value(&self) -> bool {
        self.typed_value.is_some()
    }

    /// Whether the pointer is over what is shown.
    pub fn pointer_over(&self, window: &Window) -> bool {
        let any_offered = self.offered_groups != SlidersOffered::NoneOffered;
        any_offered && window.cursor_position().is_some_and(|pointer_position| pointer_position.x >= window.width() - SLIDERS_MARGIN_PIXELS - self.shown_width() && pointer_position.y <= self.shown_foot() + SLIDERS_MARGIN_PIXELS - self.scrolled_pixels)
    }

    /// Offers `to_offer`: hidden, nothing is held or typed; a group not
    /// offered is closed.
    pub fn offer_groups(&mut self, to_offer: SlidersOffered) {
        if to_offer == self.offered_groups {
            return;
        }
        (self.offered_groups, self.button_held, self.dragged_slider, self.typed_value) = (to_offer, false, None, None);
        if let SlidersShown::OneGroup(shown_group) = self.what_shown
            && !self.listed_groups().any(|listed_group| listed_group == shown_group)
        {
            self.show_from_top(SlidersShown::ClosedButton);
        }
    }

    /// Shows `to_show`, from its top.
    fn show_from_top(&mut self, to_show: SlidersShown) {
        (self.what_shown, self.scrolled_pixels) = (to_show, 0.0);
    }

    /// The groups the menu lists.
    fn listed_groups(&self) -> impl Iterator<Item = SliderGroup> + use<> {
        let every_group_offered = self.offered_groups == SlidersOffered::EveryGroup;
        SLIDER_GROUPS.into_iter().filter(move |tuning_group| every_group_offered || !tuning_group.setup_only())
    }

    /// The group shown, if one is.
    fn shown_group(&self) -> Option<SliderGroup> {
        match self.what_shown {
            SlidersShown::OneGroup(shown_group) => Some(shown_group),
            _ => None,
        }
    }

    /// How many rows what is shown has under its first: the menu's
    /// groups, or a group's sliders.
    fn rows_under(&self) -> usize {
        match self.what_shown {
            SlidersShown::ClosedButton => 0,
            SlidersShown::GroupMenu => self.listed_groups().count(),
            SlidersShown::OneGroup(shown_group) => group_rows(shown_group).count(),
        }
    }

    /// Where what is shown ends, from the window's top, not scrolled.
    fn shown_foot(&self) -> f32 {
        SLIDERS_MARGIN_PIXELS + ROW_STEP_PIXELS * (1 + self.rows_under()) as f32 + 4.0
    }

    /// Screen pixels what is shown takes across.
    fn shown_width(&self) -> f32 {
        if self.what_shown == SlidersShown::ClosedButton { CLOSED_BUTTON_WIDTH_PIXELS } else { PANEL_WIDTH_PIXELS }
    }

    /// The row the pointer is on, the first 0, if it is over what is shown.
    fn row_under(&self, window: &Window) -> Option<usize> {
        let pointer_position = window.cursor_position().filter(|_| self.pointer_over(window))?;
        let pixels_down = pointer_position.y + self.scrolled_pixels - SLIDERS_MARGIN_PIXELS;
        (pixels_down >= 0.0).then_some((pixels_down / ROW_STEP_PIXELS) as usize).filter(|&row_number| row_number <= self.rows_under())
    }
}

/// The numbers of `tuning_group`, by their places in [`NAMES`], a row each,
/// in the order the sliders' file has them.
fn group_rows(tuning_group: SliderGroup) -> impl Iterator<Item = usize> {
    let mut slider_indices: Vec<usize> = (0..NAMES.len()).filter(|&slider_index| tuned_number(slider_index).slider_group == tuning_group).collect();
    slider_indices.sort_unstable_by_key(|&sorted_index| tuned_number(sorted_index).line);
    slider_indices.into_iter()
}

/// The middle of the `row_number`-th row, the first 0, from the window's top.
fn row_middle(row_number: usize) -> f32 {
    SLIDERS_MARGIN_PIXELS + ROW_STEP_PIXELS * (row_number as f32 + 0.5) - 4.0
}

/// Anything of the sliders: what it is shown with, and whether only
/// while every group is offered (`Some(true)`), only while not
/// (`Some(false)`), or either way.
#[derive(Component)]
pub struct Part {
    /// What it is shown with.
    shown_with: SlidersShown,
    /// Whether it is shown only while every group is offered, or only
    /// while not.
    every_group_only: Option<bool>,
}

/// How far down the window a part is, not scrolled.
#[derive(Component)]
pub struct PartTop(f32);

/// Opens the menu by `U`, the sliders closed; closes whatever is open.
pub fn toggle(pressed_keys: Res<ButtonInput<KeyCode>>, mut sliders: ResMut<Sliders>) {
    if pressed_keys.just_pressed(KeyCode::KeyU) && !sliders.typing_value() && sliders.offered_groups != SlidersOffered::NoneOffered {
        let to_show = if sliders.what_shown == SlidersShown::ClosedButton { SlidersShown::GroupMenu } else { SlidersShown::ClosedButton };
        sliders.show_from_top(to_show);
    }
}

/// Scrolls what is shown by the wheel, the pointer over it: no
/// further than its last row at the window's foot. And shows it, of
/// all the parts.
pub fn scroll_sliders(mut sliders: ResMut<Sliders>, wheel: Res<AccumulatedMouseScroll>, window: Single<&Window>, mut parts: Query<(&Part, &PartTop, &mut Node, &mut Visibility)>) {
    let most_scrolled = (sliders.shown_foot() + SLIDERS_MARGIN_PIXELS - window.height()).max(0.0);
    let wheel_moved = if sliders.pointer_over(&window) { wheel.delta.y * NOTCH_SCROLL_PIXELS } else { 0.0 };
    let scrolled_now = (sliders.scrolled_pixels - wheel_moved).clamp(0.0, most_scrolled);
    if scrolled_now != sliders.scrolled_pixels {
        sliders.scrolled_pixels = scrolled_now;
    }
    let every_group_offered = sliders.offered_groups == SlidersOffered::EveryGroup;
    for (part, part_top, mut node, mut part_visibility) in &mut parts {
        let part_shown = sliders.offered_groups != SlidersOffered::NoneOffered && part.shown_with == sliders.what_shown && part.every_group_only.is_none_or(|only_every_group| only_every_group == every_group_offered);
        part_visibility.set_if_neq(if part_shown { Visibility::Visible } else { Visibility::Hidden });
        if node.top != Val::Px(part_top.0 - scrolled_now) {
            node.top = Val::Px(part_top.0 - scrolled_now);
        }
    }
}
