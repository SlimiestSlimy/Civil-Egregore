//! The world drawn: its types frozen as the server names them, frames
//! asked for a few superchunks at a time -- the client going round the
//! viewport's hot superchunks, the server never asked for more than it
//! answers between ticks -- each superchunk painted on Bevy's task
//! threads at the detail level of the zoom, and shown as a sprite a
//! render layer, stacked by layer.

use super::camera::{drawn_at, seen_by};
use super::windows::WorldTypesFrozen;
use super::WindowServer;
use crate::painting::{Picture, RenderRules};
use crate::server_end::Shown;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::AsyncComputeTaskPool;
use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use protocol::world_types::{of_a_saved_world, RegisterTypes};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use utilities::coordinates::SUPERCHUNK_SIDE_CELLS;

/// Superchunks a frame is asked for at most: the server copies them
/// between its ticks.
const SUPERCHUNKS_AN_ASK: u32 = 4;
/// Superchunks being painted at most before more are asked for: the
/// window's own work kept from piling up.
const MOST_BEING_PAINTED: usize = 8;
/// How long an ask goes unanswered before it is taken as lost: asked
/// of no world, say.
const ASK_GIVEN_UP: Duration = Duration::from_secs(2);

/// A superchunk painted: of which world, where, and each render layer's picture.
struct Painted {
    /// The world it is of.
    world: u64,
    /// Where it is, `(x, y)` in superchunks.
    superchunk_cartesian: (u32, u32),
    /// Its drawn cells are `2^detail_level` cells a side.
    detail_level: u32,
    /// Each render layer's picture, lowest first.
    pictures: Vec<Picture>,
}

/// The world as the window draws it.
#[derive(Resource)]
pub struct Drawing {
    /// What registers the game's types: each world's frozen with it.
    register_types: RegisterTypes,
    /// The world drawn, the server's count of it, and its render rules.
    world: Option<(u64, Arc<RenderRules>)>,
    /// When the ask being answered was made, if one is.
    asked_at: Option<Instant>,
    /// The viewport's hot superchunks the next ask passes over: where
    /// the going round has come to.
    round_place: u32,
    /// Superchunks being painted.
    being_painted: Arc<AtomicUsize>,
    /// Where a superchunk painted is sent, by the thread that painted it.
    painted_sender: Sender<Painted>,
    /// The superchunks painted, as they come.
    painted: Mutex<Receiver<Painted>>,
    /// Each superchunk drawn, `(x, y)`: its sprites, one a render layer.
    sprites: HashMap<(u32, u32), Vec<Entity>>,
    /// The last frame, its cells taken out: what the window's title says.
    pub last_frame: Option<ViewportFrame>,
}

impl Drawing {
    /// Nothing drawn yet, each world's types registered by `register_types`.
    pub fn new(register_types: RegisterTypes) -> Self {
        let (painted_sender, painted) = channel();
        Self { register_types, world: None, asked_at: None, round_place: 0, being_painted: Arc::new(AtomicUsize::new(0)), painted_sender, painted: Mutex::new(painted), sprites: HashMap::new(), last_frame: None }
    }
}

/// What the server shows taken in: a world's types -- the world drawn
/// afresh, its windows told ([`WorldTypesFrozen`]) -- and frames, each superchunk of them sent to be painted.
pub fn take_shown(window_server: Res<WindowServer>, mut drawing: ResMut<Drawing>, mut bevy_commands: Commands) {
    let shown = window_server.shown.lock().expect("what the server shows");
    while let Ok(shown) = shown.try_recv() {
        match shown {
            Shown::WorldTypes(world, types_text) => {
                for (_, sprites) in drawing.sprites.drain() {
                    sprites.into_iter().for_each(|sprite| bevy_commands.entity(sprite).despawn());
                }
                drawing.world = match of_a_saved_world(&types_text, drawing.register_types) {
                    Ok(frozen_type_registry) => {
                        let render_rules = Arc::new(RenderRules::of(&frozen_type_registry));
                        bevy_commands.trigger(WorldTypesFrozen { types_text, frozen_type_registry: Arc::new(frozen_type_registry) });
                        Some((world, render_rules))
                    }
                    Err(why_not) => {
                        error!("the world's types do not register here: {why_not}");
                        None
                    }
                };
                (drawing.asked_at, drawing.round_place) = (None, 0);
            }
            Shown::Frame(frame) => take_frame(*frame, &mut drawing, &mut bevy_commands),
        }
    }
}

/// `frame` taken in: its superchunks sent to be painted, the sprites of
/// those cold or out of view now dropped.
fn take_frame(mut frame: ViewportFrame, drawing: &mut Drawing, bevy_commands: &mut Commands) {
    let Some((world, render_rules)) = drawing.world.clone().filter(|(world, _)| *world == frame.world) else {
        return;
    };
    for cells in std::mem::take(&mut frame.cells) {
        let (painted_sender, being_painted, render_rules, detail_level) = (drawing.painted_sender.clone(), Arc::clone(&drawing.being_painted), Arc::clone(&render_rules), frame.pixel_side_log2);
        being_painted.fetch_add(1, Ordering::Relaxed);
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let pictures = render_rules.paint(&cells, detail_level);
                // The window gone, nothing waits for it.
                _ = painted_sender.send(Painted { world, superchunk_cartesian: cells.superchunk_cartesian, detail_level, pictures });
                being_painted.fetch_sub(1, Ordering::Relaxed);
            })
            .detach();
    }
    drawing.sprites.retain(|superchunk_cartesian, sprites| {
        let still_shown = frame.hot.contains(superchunk_cartesian);
        if !still_shown {
            sprites.iter().for_each(|&sprite| bevy_commands.entity(sprite).despawn());
        }
        still_shown
    });
    if !frame.more_to_come {
        drawing.asked_at = None;
        drawing.round_place += SUPERCHUNKS_AN_ASK;
        if drawing.round_place as usize >= frame.hot.len() {
            drawing.round_place = 0;
        }
    }
    drawing.last_frame = Some(frame);
}

/// The next superchunks asked for, once the last ask is answered and
/// the window has painted what it was sent: those of the camera's view
/// the going round has come to, at the camera's detail level.
pub fn ask_frames(window_server: Res<WindowServer>, mut drawing: ResMut<Drawing>, window: Single<&Window>, camera: Single<(&Transform, &Projection), With<Camera2d>>) {
    let (Some((_, render_rules)), (transform, Projection::Orthographic(orthographic))) = (&drawing.world, *camera) else {
        return;
    };
    if drawing.asked_at.is_some_and(|asked_at| asked_at.elapsed() < ASK_GIVEN_UP) || drawing.being_painted.load(Ordering::Relaxed) >= MOST_BEING_PAINTED {
        return;
    }
    let (viewport, detail_level) = seen_by(transform, orthographic, window.size());
    let frame_ask = FrameAsk { viewport: Some(viewport), pixel_side_log2: detail_level, superchunks_skipped: drawing.round_place, most_superchunks: SUPERCHUNKS_AN_ASK, seen_near: None, planes: render_rules.planes.clone(), counted: Vec::new() };
    if window_server.server_end.link.ask_frame(frame_ask) {
        drawing.asked_at = Some(Instant::now());
    }
}

/// The superchunks painted shown: each render layer's picture its
/// sprite's image, a sprite hidden where nothing was painted.
pub fn show_painted(mut drawing: ResMut<Drawing>, mut images: ResMut<Assets<Image>>, mut sprites: Query<(&mut Sprite, &mut Visibility)>, mut bevy_commands: Commands) {
    let drawing = &mut *drawing;
    let (Some((world, render_rules)), painted) = (&drawing.world, drawing.painted.lock().expect("the superchunks painted")) else {
        return;
    };
    while let Ok(Painted { world: painted_world, superchunk_cartesian, detail_level, pictures }) = painted.try_recv() {
        if painted_world != *world {
            continue;
        }
        let images_of: Vec<Option<Handle<Image>>> = pictures.into_iter().map(|picture| picture.map(|picture| images.add(picture_image(picture, detail_level)))).collect();
        if let Some(superchunk_sprites) = drawing.sprites.get(&superchunk_cartesian) {
            for (&sprite, image) in superchunk_sprites.iter().zip(images_of) {
                // A sprite spawned this frame is not queried until the next: painted on the next round.
                let Ok((mut sprite, mut visibility)) = sprites.get_mut(sprite) else {
                    continue;
                };
                match image {
                    Some(image) => (sprite.image, *visibility) = (image, Visibility::Inherited),
                    None => *visibility = Visibility::Hidden,
                }
            }
            continue;
        }
        let (x, y) = superchunk_cartesian;
        let middle = drawn_at(u64::from(x * SUPERCHUNK_SIDE_CELLS), u64::from(y * SUPERCHUNK_SIDE_CELLS)) + Vec2::new(1.0, -1.0) * (SUPERCHUNK_SIDE_CELLS / 2) as f32;
        let spawned = render_rules.render_layers().zip(images_of).map(|(render_layer, image)| {
            let visibility = if image.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            bevy_commands.spawn((Sprite { image: image.unwrap_or_default(), custom_size: Some(Vec2::splat(SUPERCHUNK_SIDE_CELLS as f32)), ..default() }, Transform::from_translation(middle.extend(render_layer as f32)), visibility)).id()
        });
        drawing.sprites.insert(superchunk_cartesian, spawned.collect());
    }
}

/// `picture`, a superchunk's at `detail_level`, as an image: its
/// drawn cells sharp-edged however near the camera.
fn picture_image(picture: Vec<u8>, detail_level: u32) -> Image {
    let side = SUPERCHUNK_SIDE_CELLS >> detail_level;
    let mut image = Image::new(Extent3d { width: side, height: side, depth_or_array_layers: 1 }, TextureDimension::D2, picture, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::nearest();
    image
}
