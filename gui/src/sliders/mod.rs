//! Sliders over the window's top right corner, one a number of
//! `utilities::tuning`, in groups, one group on the screen at a time,
//! opened from a menu (`docs/gui.md`, "Sliders").

mod slide;
mod spawn;
mod tell;

pub use slide::slide;
pub use spawn::spawn;
pub use tell::tell;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use utilities::tuning::{tuned, Group, GROUPS, NAMES};

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
/// Screen pixels the button shown while the sliders are closed is across.
const CLOSED: f32 = 96.0;
/// Screen pixels a notch of the wheel scrolls a page.
const NOTCH: f32 = 2.0 * ROW;

/// What of the sliders is on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shown {
    /// Only the button that opens the menu.
    #[default]
    Closed,
    /// The menu: the groups, a row each.
    Menu,
    /// One group's sliders.
    Group(Group),
}

/// Which of the sliders are offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Offered {
    /// None: none is shown, and none takes the pointer or the keys.
    #[default]
    Hidden,
    /// Every group: a world is being set up.
    Everything,
    /// Every group but those only of setting a world up: a world runs.
    Running,
}

/// The sliders: what of them is shown, and what the pointer and the
/// keys are doing with them.
#[derive(Resource, Default)]
pub struct Sliders {
    /// What is offered.
    offered: Offered,
    /// What is shown of it.
    shown: Shown,
    /// Screen pixels what is shown is scrolled up by.
    scrolled: f32,
    /// Whether the left button went down over them and is still held:
    /// the view is then not dragged, wherever the pointer goes.
    held: bool,
    /// The slider being dragged, if one is.
    dragged: Option<usize>,
    /// The number whose value is being typed, and what is typed so far.
    typed: Option<(usize, String)>,
    /// Where the pointer last was, and the seconds it has rested there.
    rested: (Vec2, f32),
}

impl Sliders {
    /// Whether they have the left button: it went down over them.
    pub fn held(&self) -> bool {
        self.held
    }

    /// Whether a value is being typed: the keys are theirs.
    pub fn typing(&self) -> bool {
        self.typed.is_some()
    }

    /// Whether the pointer is over what is shown.
    pub fn over(&self, window: &Window) -> bool {
        let shown = self.offered != Offered::Hidden;
        shown && window.cursor_position().is_some_and(|pointer| pointer.x >= window.width() - MARGIN - self.across() && pointer.y <= self.foot() + MARGIN - self.scrolled)
    }

    /// Offers `offered`: hidden, nothing is held or typed; a group not
    /// offered is closed.
    pub fn offer(&mut self, offered: Offered) {
        if offered == self.offered {
            return;
        }
        (self.offered, self.held, self.dragged, self.typed) = (offered, false, None, None);
        if let Shown::Group(group) = self.shown
            && !self.listed().any(|listed| listed == group)
        {
            self.show(Shown::Closed);
        }
    }

    /// Shows `what`, from its top.
    fn show(&mut self, what: Shown) {
        (self.shown, self.scrolled) = (what, 0.0);
    }

    /// The groups the menu lists.
    fn listed(&self) -> impl Iterator<Item = Group> + use<> {
        let everything = self.offered == Offered::Everything;
        GROUPS.into_iter().filter(move |group| everything || !group.setup_only())
    }

    /// The group shown, if one is.
    fn page(&self) -> Option<Group> {
        match self.shown {
            Shown::Group(group) => Some(group),
            _ => None,
        }
    }

    /// How many rows what is shown has under its first: the menu's
    /// groups, or a group's sliders.
    fn rows_under(&self) -> usize {
        match self.shown {
            Shown::Closed => 0,
            Shown::Menu => self.listed().count(),
            Shown::Group(group) => rows(group).count(),
        }
    }

    /// Where what is shown ends, from the window's top, not scrolled.
    fn foot(&self) -> f32 {
        MARGIN + ROW * (1 + self.rows_under()) as f32 + 4.0
    }

    /// Screen pixels what is shown takes across.
    fn across(&self) -> f32 {
        if self.shown == Shown::Closed { CLOSED } else { PANEL }
    }

    /// The row the pointer is on, the first 0, if it is over what is shown.
    fn row_under(&self, window: &Window) -> Option<usize> {
        let pointer = window.cursor_position().filter(|_| self.over(window))?;
        let down = pointer.y + self.scrolled - MARGIN;
        (down >= 0.0).then_some((down / ROW) as usize).filter(|&row| row <= self.rows_under())
    }
}

/// The numbers of `group`, by their places in [`NAMES`], a row each,
/// in the order the sliders' file has them.
fn rows(group: Group) -> impl Iterator<Item = usize> {
    let mut rows: Vec<usize> = (0..NAMES.len()).filter(|&index| tuned(index).group == group).collect();
    rows.sort_unstable_by_key(|&index| tuned(index).line);
    rows.into_iter()
}

/// The middle of the `row`-th row, the first 0, from the window's top.
fn middle(row: usize) -> f32 {
    MARGIN + ROW * (row as f32 + 0.5) - 4.0
}

/// Anything of the sliders: what it is shown with, and whether only
/// while every group is offered (`Some(true)`), only while not
/// (`Some(false)`), or either way.
#[derive(Component)]
pub struct Part {
    /// What it is shown with.
    shown: Shown,
    /// Whether it is shown only while every group is offered, or only
    /// while not.
    everything: Option<bool>,
}

/// How far down the window a part is, not scrolled.
#[derive(Component)]
pub struct Row(f32);

/// Opens the menu by `U`, the sliders closed; closes whatever is open.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut sliders: ResMut<Sliders>) {
    if keys.just_pressed(KeyCode::KeyU) && !sliders.typing() && sliders.offered != Offered::Hidden {
        let what = if sliders.shown == Shown::Closed { Shown::Menu } else { Shown::Closed };
        sliders.show(what);
    }
}

/// Scrolls what is shown by the wheel, the pointer over it: no
/// further than its last row at the window's foot. And shows it, of
/// all the parts.
pub fn scroll(mut sliders: ResMut<Sliders>, wheel: Res<AccumulatedMouseScroll>, window: Single<&Window>, mut parts: Query<(&Part, &Row, &mut Node, &mut Visibility)>) {
    let most = (sliders.foot() + MARGIN - window.height()).max(0.0);
    let moved = if sliders.over(&window) { wheel.delta.y * NOTCH } else { 0.0 };
    let now = (sliders.scrolled - moved).clamp(0.0, most);
    if now != sliders.scrolled {
        sliders.scrolled = now;
    }
    let everything = sliders.offered == Offered::Everything;
    for (part, row, mut node, mut visibility) in &mut parts {
        let shown = sliders.offered != Offered::Hidden && part.shown == sliders.shown && part.everything.is_none_or(|only| only == everything);
        visibility.set_if_neq(if shown { Visibility::Visible } else { Visibility::Hidden });
        if node.top != Val::Px(row.0 - now) {
            node.top = Val::Px(row.0 - now);
        }
    }
}
