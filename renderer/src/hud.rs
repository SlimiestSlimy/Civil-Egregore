//! The text over the world: what the last frame said of it, how it is
//! drawn, and the keys.

use crate::link::{Link, Seen};
use bevy::prelude::*;
use gui::Screen;

/// The text over the world.
#[derive(Component)]
pub struct Hud;

/// `number` with its digits in threes: 1,234,567.
fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (place, digit) in digits.chars().enumerate() {
        if place > 0 && (digits.len() - place).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// The text, in the window's top left corner.
pub fn spawn(mut commands: Commands) {
    let corner = Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(8.0), padding: UiRect::all(Val::Px(6.0)), ..default() };
    commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(13.0), ..default() }, corner, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), Hud));
}

/// The keys, as the text says them.
const KEYS: &str = "move: arrows, WASD, drag   zoom: wheel, Q E   space: pause\nT: flat out   [ ]: pace   F11: fullscreen   B: superchunks   C: chunks   H: heights   P: lines (on the map)   U: sliders   Escape: options";

/// Writes what the last frame said over the world -- nothing over the
/// main menu.
pub fn hud(hud: Single<(&mut Text, &mut Visibility), With<Hud>>, seen: Res<Seen>, link: Res<Link>, screen: Res<Screen>) {
    let (mut text, mut visibility) = hud.into_inner();
    visibility.set_if_neq(if *screen == Screen::World { Visibility::Visible } else { Visibility::Hidden });
    let Some(frame) = &seen.frame else {
        text.0 = format!("no world runs yet\n{KEYS}");
        return;
    };
    let pace = match (link.paused, link.pace) {
        (true, _) => "paused".to_string(),
        (false, Some(pace)) => format!("held to {pace} ticks a second"),
        (false, None) => "flat out".to_string(),
    };
    let drawn = match seen.near_pixels {
        0 if seen.map > 0 => format!("the map, a pixel {} cells a side", seen.map),
        0 => format!("a pixel {} cell(s) a side", 1u32 << seen.detail),
        pixels => format!("a cell {pixels} pixels a side"),
    };
    let said = frame.said.as_ref().map_or(String::new(), |said| format!("{said}\n"));
    let size = frame.side.map_or("no end".to_string(), |side| format!("{side} superchunks a side"));
    text.0 = format!(
        "{said}seed {}   {size}   ocean at {}   tick {}\n{} ticks a second ({pace})\n{} sheep   {} cells of grass   {} trees\n{} superchunk(s) in view, {drawn}\na frame, {} of them: {:.0} us of the host ({:.2}% of its time), {:.1} ms painting\n{KEYS}",
        utilities::seed::hex(frame.seed),
        frame.generation.shape.ocean,
        grouped(frame.tick),
        grouped(frame.ticks_a_second as u64),
        grouped(frame.sheep as u64),
        grouped(frame.grass),
        grouped(frame.trees),
        seen.in_view,
        seen.painted,
        frame.sync_seconds * 1e6,
        frame.sync_share * 100.0,
        seen.paint_seconds * 1e3
    );
}
