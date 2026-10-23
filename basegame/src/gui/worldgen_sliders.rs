//! A worldgen parameter's slider: its name, and a slider over its range,
//! reading the value the world is reset with as it is built and setting
//! it as it is slid -- the world itself changed only by a reset.

use crate::worldgen::calls::WorldgenCalls;
use crate::worldgen::generation::SHEEP_PER_SUPERCHUNK;
use crate::worldgen::mesh::SIGMOID_ONE;
use crate::worldgen::FRACTION_ONE;
use bevy::feathers::controls::FeathersSlider;
use bevy::feathers::constants::{fonts, size};
use bevy::feathers::font_styles::InheritableFont;
use bevy::feathers::theme::ThemedText;
use bevy::prelude::*;
use bevy::ui_widgets::{slider_self_update, SliderPrecision, SliderStep, ValueChange};
use client::client_api::gui::GuiCode;
use client::client_api::ServerCalls;

/// The world generation's window's grid, in blocks across and down: two
/// columns of sliders.
pub const WINDOW_BLOCKS: (u32, u32) = (2 * SLIDER_BLOCKS.0, 14);
/// A slider's place, in blocks across and down.
const SLIDER_BLOCKS: (u32, u32) = (12, 1);
/// Pixels the parameter's name takes beside its slider.
const NAME_PIXELS: f32 = 150.0;

/// How a parameter is slid: from least to most, a slid unit so many of
/// its own, whole numbers or not.
#[derive(Clone, Copy, Debug)]
struct Slid {
    /// The least it is slid to.
    least: f32,
    /// The most.
    most: f32,
    /// What one of the slider's units is of the parameter's.
    unit: f32,
    /// Whether it is slid in whole units.
    whole: bool,
}

/// A whole number, slid as it is.
const fn whole(least: f32, most: f32) -> Slid {
    Slid { least, most, unit: 1.0, whole: true }
}

/// A share, `FRACTION_ONE` the whole.
const fn share(most: f32) -> Slid {
    Slid { least: 0.0, most, unit: FRACTION_ONE as f32, whole: false }
}

/// A sigmoidness, `SIGMOID_ONE` an even slope.
const SIGMOIDNESS: Slid = Slid { least: 1.0, most: 16.0, unit: SIGMOID_ONE as f32, whole: false };

/// How each parameter is slid, by its name.
const SLIDS: [(&str, Slid); 28] = [
    ("ocean floor", whole(0.0, 4096.0)),
    ("ocean level", whole(0.0, 8192.0)),
    ("vertex spacing", whole(6.0, 18.0)),
    ("ocean share", share(1.0)),
    ("highest land", whole(0.0, 8192.0)),
    ("clumping", share(1.0)),
    ("coast breadth", whole(0.0, 4.0)),
    ("coast lowness", SIGMOIDNESS),
    ("narrowest blend", share(1.0)),
    ("widest blend", share(1.0)),
    ("least sigmoid", SIGMOIDNESS),
    ("most sigmoid", SIGMOIDNESS),
    ("line bending", share(1.0)),
    ("finer mesh depth", whole(0.0, 10.0)),
    ("finer mesh share", share(1.0)),
    ("finer mesh height", whole(0.0, 2048.0)),
    ("finer mesh falloff", share(1.0)),
    ("weight spread", share(1.0)),
    ("raised share", share(1.0)),
    ("grass cover", share(1.0)),
    ("grass patch size", whole(1.0, 10.0)),
    ("grass patch detail", share(2.0)),
    ("grass scatter", share(2.0)),
    ("tree cover", share(1.0)),
    ("tree patch size", whole(1.0, 10.0)),
    ("tree patch detail", share(2.0)),
    ("tree scatter", share(2.0)),
    (SHEEP_PER_SUPERCHUNK, whole(0.0, 16000.0)),
];

/// The slider of the worldgen parameter `parameter_name`, at `place`
/// among them, read and set through `worldgen_calls`.
pub fn slider(parameter_name: &str, place: u64, worldgen_calls: WorldgenCalls) -> GuiCode {
    let slid = SLIDS.iter().find(|(slid_name, _)| *slid_name == parameter_name).map_or(whole(0.0, 1.0), |(_, slid)| *slid);
    let shown_name = parameter_name.to_string();
    GuiCode::new(SLIDER_BLOCKS, move |slider_place, gui_context| {
        let value_now = gui_context.server_calls.call(worldgen_calls.parameter, &[place]).ok().and_then(|answer| answer.first().copied()).unwrap_or(0);
        let (step, precision): (f32, i32) = if slid.whole { (1.0, 0) } else { (0.01, 2) };
        let shown_value = ((value_now as f32 / slid.unit / step).round() / step.recip()).clamp(slid.least, slid.most);
        let name = shown_name.clone();
        slider_place.apply_scene(bsn! {
            Node { flex_grow: 1.0, align_items: AlignItems::Center, column_gap: px(6) }
            Children [
                (Node { width: px(NAME_PIXELS) } InheritableFont { font: fonts::REGULAR, font_size: size::COMPACT_FONT, weight: FontWeight::NORMAL } Children [(Text({name.clone()}) ThemedText)]),
                (
                    @FeathersSlider { @value: {shown_value}, @min: {slid.least}, @max: {slid.most} }
                    SliderStep({step})
                    SliderPrecision({precision})
                    on(slider_self_update)
                    on(move |change: On<ValueChange<f32>>, server_calls: Res<ServerCalls>| {
                        let value = (change.value * slid.unit).round().max(0.0) as u64;
                        if let Err(why_not) = server_calls.call(worldgen_calls.set_parameter, &[place, value]) {
                            warn!("setting a worldgen parameter: {why_not}");
                        }
                    })
                )
            ]
        });
    })
}
