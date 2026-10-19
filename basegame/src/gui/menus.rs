//! The windows' code and their buttons': a window builds nothing of its
//! own, its guis fill it; a button opens a window, or calls a function
//! of the server's.

use bevy::feathers::controls::FeathersButton;
use bevy::feathers::theme::ThemedText;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use client::client_api::gui::{GuiCode, OpenWindow};
use client::client_api::ServerCalls;
use protocol::type_registry::{ClientCallType, GuiType};

/// The world's window's grid, in blocks across and down: a button a row.
pub const WORLD_WINDOW_BLOCKS: (u32, u32) = (8, 2);
/// A button's place, in blocks across and down.
const BUTTON_BLOCKS: (u32, u32) = (8, 1);

/// A window of a grid `blocks` across and down: its guis fill it.
pub fn window(blocks: (u32, u32)) -> GuiCode {
    GuiCode::new(blocks, |_, _| {})
}

/// A button saying `caption`, opening the window `window`.
pub fn opening_button(caption: &'static str, window: GuiType) -> GuiCode {
    GuiCode::new(BUTTON_BLOCKS, move |place, _| {
        place.apply_scene(bsn! {
            Node { flex_grow: 1.0 }
            Children [(
                @FeathersButton
                Node { flex_grow: 1.0 }
                on(move |_: On<Activate>, mut bevy_commands: Commands| bevy_commands.trigger(OpenWindow(window)))
                Children [(Text({caption}) ThemedText)]
            )]
        });
    })
}

/// A button saying `caption`, calling the server's function `call` with no words.
pub fn calling_button(caption: &'static str, call: ClientCallType) -> GuiCode {
    GuiCode::new(BUTTON_BLOCKS, move |place, _| {
        place.apply_scene(bsn! {
            Node { flex_grow: 1.0 }
            Children [(
                @FeathersButton
                Node { flex_grow: 1.0 }
                on(move |_: On<Activate>, server_calls: Res<ServerCalls>| {
                    if let Err(why_not) = server_calls.call(call, &[]) {
                        warn!("{caption}: {why_not}");
                    }
                })
                Children [(Text({caption}) ThemedText)]
            )]
        });
    })
}
