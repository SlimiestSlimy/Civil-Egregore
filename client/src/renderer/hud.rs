//! The text over the world: what the last frame said of it, how it is
//! drawn, and the keys.

use crate::renderer::link::{HostLink, LastFrameSeen};
use bevy::prelude::*;
use crate::gui::ShownScreen;
use basegame::base_types::BaseTypes;
use protocol::type_registry::LayerType;

/// The layers whose cells the text counts, of `base_types`, asked of
/// every frame ([`protocol::pipe::frame::FrameAsk::counted`]): its
/// grass, and its trees.
pub const fn counted_layers(base_types: &BaseTypes) -> [LayerType; 2] {
    [base_types.grass, base_types.tree]
}

/// The text over the world.
#[derive(Component)]
pub struct HudText;

/// `whole_number` with its digits in threes: 1,234,567.
fn grouped_in_threes(whole_number: u64) -> String {
    let decimal_digits = whole_number.to_string();
    let mut grouped_text = String::with_capacity(decimal_digits.len() + decimal_digits.len() / 3);
    for (place, decimal_digit) in decimal_digits.chars().enumerate() {
        if place > 0 && (decimal_digits.len() - place).is_multiple_of(3) {
            grouped_text.push(',');
        }
        grouped_text.push(decimal_digit);
    }
    grouped_text
}

/// The text, in the window's top left corner.
pub fn spawn_hud(mut bevy_commands: Commands) {
    let corner_node = Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(8.0), padding: UiRect::all(Val::Px(6.0)), ..default() };
    bevy_commands.spawn((Text::new(""), TextFont { font_size: FontSize::Px(13.0), ..default() }, corner_node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), HudText));
}

/// The keys, as the text says them.
const KEYS_TEXT: &str = "move: arrows, WASD, drag   zoom: wheel, Q E   space: pause\nT: flat out   [ ]: pace   F11: fullscreen   B: superchunks   C: chunks   H: heights   M: map   P: lines (on the map)   U: sliders   Escape: options";

/// Writes what the last frame said over the world -- nothing over the
/// main menu.
pub fn write_hud(hud_parts: Single<(&mut Text, &mut Visibility), With<HudText>>, last_seen: Res<LastFrameSeen>, host_link: Res<HostLink>, shown_screen: Res<ShownScreen>) {
    let (mut shown_text, mut hud_visibility) = hud_parts.into_inner();
    hud_visibility.set_if_neq(if *shown_screen == ShownScreen::World { Visibility::Visible } else { Visibility::Hidden });
    let Some(last_frame) = &last_seen.last_frame else {
        shown_text.0 = format!("no world runs yet\n{KEYS_TEXT}");
        return;
    };
    let pace_said = match (host_link.world_paused, last_frame.held_pace) {
        (true, _) => "paused".to_string(),
        (false, Some(held_pace)) => format!("held to {held_pace} ticks a second"),
        (false, None) => "flat out".to_string(),
    };
    let drawn_said = match last_seen.near_pixels {
        0 if last_seen.map > 0 => format!("the map, a pixel {} cells a side", last_seen.map),
        0 => format!("a pixel {} cell(s) a side", 1u32 << last_seen.detail_shift),
        cell_pixels => format!("a cell {cell_pixels} pixels a side"),
    };
    let said_line = last_frame.last_outcome.as_ref().map_or(String::new(), |frame_said| format!("{frame_said}\n"));
    let size_said = last_frame.side_superchunks.map_or("no end".to_string(), |side_superchunks| format!("{side_superchunks} superchunks a side"));
    let counted = |layer_type: LayerType| last_frame.counts.iter().find(|(counted_type, _)| *counted_type == layer_type).map_or(0, |&(_, count)| count);
    let [grass_counted, trees_counted] = last_seen.base_types.as_ref().map_or([0; 2], |base_types| counted_layers(base_types).map(counted));
    shown_text.0 = format!(
        "{said_line}seed {}   {size_said}   ocean at {}   tick {}\n{} ticks a second ({pace_said})\n{} entities   {} cells of grass   {} trees\n{} superchunk(s) in the viewport, {} hot, {drawn_said}\na frame, {} of them: {:.0} us of the host ({:.2}% of its time), {:.1} ms painting\n{KEYS_TEXT}",
        utilities::seed::seed_as_hex(last_frame.seed),
        last_frame.drawn_heights.ocean,
        grouped_in_threes(last_frame.tick),
        grouped_in_threes(last_frame.ticks_a_second as u64),
        grouped_in_threes(last_frame.entity_count as u64),
        grouped_in_threes(grass_counted),
        grouped_in_threes(trees_counted),
        last_seen.viewport_superchunks,
        last_frame.hot.len(),
        last_seen.painted_superchunks,
        last_frame.sync_seconds * 1e6,
        last_frame.sync_share * 100.0,
        last_seen.paint_seconds * 1e3
    );
}
