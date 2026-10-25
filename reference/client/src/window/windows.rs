//! The one window system: each world's gui types gathered as its types
//! are frozen ([`WorldTypesFrozen`]) -- a window and the guis naming it
//! their parent -- and each window framed: a title bar it is dragged by
//! and closed from, and its grid of blocks, each of its guis built in
//! its place there by the gui's own code (`client_api::gui`). A window
//! hooked `ShownByDefault` opens with each world; any other when a gui
//! asks for it ([`OpenWindow`]).

use crate::client_api::gui::{GuiCode, GuiContext, OpenWindow, BLOCK_PIXELS};
use crate::client_api::ServerCalls;
use bevy::feathers::constants::{fonts, size};
use bevy::feathers::font_styles::InheritableFont;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText};
use bevy::feathers::tokens;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use protocol::type_registry::{FrozenTypeRegistry, GuiType, Hook, RegisteredType};
use std::collections::HashMap;
use std::sync::Arc;

/// Where the first window opens, from the top left, in pixels; each next
/// one a step further.
const FIRST_WINDOW_AT: Vec2 = Vec2::new(16.0, 16.0);
/// How much further each window opens than the one before it.
const NEXT_WINDOW_STEP: Vec2 = Vec2::new(28.0, 28.0);
/// The height of a window's title bar, in pixels.
const TITLE_BAR_PIXELS: f32 = 26.0;

/// A world's types frozen: the text the server named them by, and the
/// registry -- what its windows are made of.
#[derive(Event, Clone)]
pub struct WorldTypesFrozen {
    /// The types, as the server named them.
    pub types_text: String,
    /// The registry frozen of them.
    pub frozen_type_registry: Arc<FrozenTypeRegistry>,
}

/// A window's type: its title, its code, whether it opens with each
/// world, and its guis, in the order they were registered.
struct WindowType {
    /// Its type.
    gui: GuiType,
    /// Its title: its name, without its bundle's.
    title: String,
    /// What builds it: its grid's size, and what it builds there before its guis.
    code: GuiCode,
    /// Whether it opens with each world.
    shown_by_default: bool,
    /// Its guis, each its type and its code.
    guis: Vec<(GuiType, GuiCode)>,
}

/// The world's windows: each type of them, those open, and what the next
/// opened is placed by.
#[derive(Resource, Default)]
pub struct Windows {
    /// The types text the windows were made of: a world remade of the
    /// same types keeps its windows open.
    types_text: Option<String>,
    /// Each type of window.
    window_types: Vec<WindowType>,
    /// The windows open, each its root.
    open: HashMap<GuiType, Entity>,
    /// Windows opened so far: where the next opens, and how far in front.
    opened: u32,
}

/// A world's windows made of its types, those opening with it opened --
/// unless its types are those of the world before, whose windows stay.
pub fn take_world_types(world_types: On<WorldTypesFrozen>, mut windows: ResMut<Windows>, server_calls: Res<ServerCalls>, asset_server: Res<AssetServer>, mut bevy_commands: Commands) {
    if windows.types_text.as_ref() == Some(&world_types.types_text) {
        return;
    }
    for (_, window_root) in windows.open.drain() {
        bevy_commands.entity(window_root).despawn();
    }
    let frozen_type_registry = &world_types.frozen_type_registry;
    let gui_code = |registered_type: &RegisteredType| frozen_type_registry.library_of(registered_type).and_then(|library| library.downcast_ref::<GuiCode>()).cloned();
    let mut window_types: Vec<WindowType> = Vec::new();
    for registered_type in frozen_type_registry.types() {
        let RegisteredType::Gui { name, gui, parent } = registered_type else {
            continue;
        };
        // A game built headless registers its guis without their code: nothing to build.
        let Some(code) = gui_code(registered_type) else {
            continue;
        };
        match parent {
            None => {
                let shown_by_default = frozen_type_registry.hooks_of(registered_type).contains(&Hook::ShownByDefault);
                let title = name.rsplit("::").next().unwrap_or(name).to_string();
                window_types.push(WindowType { gui: *gui, title, code, shown_by_default, guis: Vec::new() });
            }
            Some(parent) => {
                if let Some(window_type) = window_types.iter_mut().find(|window_type| window_type.gui == *parent) {
                    window_type.guis.push((*gui, code));
                }
            }
        }
    }
    let shown_by_default: Vec<GuiType> = window_types.iter().filter(|window_type| window_type.shown_by_default).map(|window_type| window_type.gui).collect();
    (windows.types_text, windows.window_types, windows.opened) = (Some(world_types.types_text.clone()), window_types, 0);
    for gui in shown_by_default {
        open_window_of(gui, &mut windows, &server_calls, &asset_server, &mut bevy_commands);
    }
}

/// The window a gui asked for opened, or brought to the front.
pub fn open_window(open_asked: On<OpenWindow>, mut windows: ResMut<Windows>, server_calls: Res<ServerCalls>, asset_server: Res<AssetServer>, mut bevy_commands: Commands) {
    open_window_of(open_asked.0, &mut windows, &server_calls, &asset_server, &mut bevy_commands);
}

/// The window of type `gui` opened, or brought to the front if it is
/// open: its title in Feathers' own font.
fn open_window_of(gui: GuiType, windows: &mut Windows, server_calls: &ServerCalls, asset_server: &AssetServer, bevy_commands: &mut Commands) {
    windows.opened += 1;
    let in_front = GlobalZIndex(windows.opened as i32);
    if let Some(&window_root) = windows.open.get(&gui) {
        bevy_commands.entity(window_root).insert(in_front);
        return;
    }
    let Some(window_type) = windows.window_types.iter().find(|window_type| window_type.gui == gui) else {
        return;
    };
    let at = FIRST_WINDOW_AT + NEXT_WINDOW_STEP * (windows.open.len() as f32);
    let grid_size = Vec2::new(window_type.code.blocks.0 as f32, window_type.code.blocks.1 as f32) * BLOCK_PIXELS;
    let window_root = bevy_commands
        .spawn((Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), flex_direction: FlexDirection::Column, border: UiRect::all(px(1)), border_radius: BorderRadius::all(px(6)), ..default() }, ThemeBackgroundColor(tokens::WINDOW_BG), ThemeBorderColor(tokens::BUTTON_BG), Interaction::default(), in_front))
        .observe(bring_to_front)
        .id();
    let title_bar = bevy_commands
        .spawn((Node { height: px(TITLE_BAR_PIXELS), padding: UiRect::horizontal(px(8)), justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }, ThemeBackgroundColor(tokens::BUTTON_BG), ChildOf(window_root)))
        .observe(move |drag: On<Pointer<Drag>>, mut nodes: Query<&mut Node>| drag_window(window_root, drag.delta, &mut nodes))
        .id();
    // Feathers' font, handed down to the title bar's text.
    bevy_commands.entity(title_bar).insert(InheritableFont { font: asset_server.load(fonts::REGULAR), font_size: size::MEDIUM_FONT, weight: FontWeight::BOLD });
    bevy_commands.spawn((Text::new(window_type.title.clone()), ThemedText, ChildOf(title_bar)));
    bevy_commands.spawn((Text::new("x"), ThemedText, ChildOf(title_bar))).observe(move |_: On<Pointer<Click>>, mut windows: ResMut<Windows>, mut bevy_commands: Commands| {
        windows.open.remove(&gui);
        bevy_commands.entity(window_root).despawn();
    });
    let grid = bevy_commands.spawn((Node { width: px(grid_size.x), min_height: px(grid_size.y), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_content: AlignContent::FlexStart, ..default() }, ChildOf(window_root))).id();
    (window_type.code.build)(&mut bevy_commands.entity(grid), &GuiContext { gui, server_calls: server_calls.clone() });
    for (child_gui, child_code) in &window_type.guis {
        let place = Vec2::new(child_code.blocks.0 as f32, child_code.blocks.1 as f32) * BLOCK_PIXELS;
        let cell = bevy_commands.spawn((Node { width: px(place.x), height: px(place.y), padding: UiRect::all(px(2)), ..default() }, ChildOf(grid))).id();
        let filling = bevy_commands.spawn((Node { flex_grow: 1.0, ..default() }, ChildOf(cell))).id();
        (child_code.build)(&mut bevy_commands.entity(filling), &GuiContext { gui: *child_gui, server_calls: server_calls.clone() });
    }
    windows.open.insert(gui, window_root);
}

/// The window pressed on brought to the front.
fn bring_to_front(press: On<Pointer<Press>>, mut windows: ResMut<Windows>, mut bevy_commands: Commands) {
    windows.opened += 1;
    bevy_commands.entity(press.entity).insert(GlobalZIndex(windows.opened as i32));
}

/// The window whose root is `window_root` moved by `delta`, in pixels.
fn drag_window(window_root: Entity, delta: Vec2, nodes: &mut Query<&mut Node>) {
    let Ok(mut node) = nodes.get_mut(window_root) else {
        return;
    };
    let moved = |at: Val, by: f32| match at {
        Val::Px(pixels) => px(pixels + by),
        other => other,
    };
    (node.left, node.top) = (moved(node.left, delta.x), moved(node.top, delta.y));
}

/// Whether the pointer is over a window, or pressed on one: the camera
/// then left alone.
pub fn pointer_on_a_window(window_roots: Query<&Interaction, With<GlobalZIndex>>) -> bool {
    window_roots.iter().any(|interaction| *interaction != Interaction::None)
}

/// No gui keeps the keys' focus once clicked: the keys are the world's
/// -- Space pausing, not pressing the button last clicked.
pub fn let_go_of_focus(mut input_focus: ResMut<InputFocus>) {
    if input_focus.get().is_some() {
        input_focus.clear();
    }
}
