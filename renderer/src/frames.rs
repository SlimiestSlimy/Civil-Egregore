//! Frames asked of the host and shown: each time the window has shown
//! one, it asks for the superchunks now in view ([`ask`]), and lays
//! what comes back painted where it is of ([`show`]) -- a superchunk's
//! image a sprite each, and from near one picture of the cells in view.

use crate::link::{Link, Seen};
use crate::paint::BROWN;
use crate::view::{first_view, Sprites, FARTHEST, SPRITE_SIDE};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use coordinates::SUPERCHUNK_SIDE_CELLS;
use server::host::frame::{Ask, Near, Viewport};
use server::host::Request;

/// Seconds from one frame asked for to the next, at least: no oftener
/// than a screen shows them.
const SYNC_EVERY: f32 = 1.0 / 60.0;
/// The coarsest the world is drawn: a pixel `2^6` cells a side, a
/// superchunk 16 pixels.
const COARSEST: u32 = 6;
/// Pixels along the side of an image kept when its superchunk goes out
/// of view, at most: a finer one is dropped, to be asked for again.
const KEPT_SIDE: u32 = 64;
/// Cells a screen pixel at most for the view to be from near: a cell
/// two pixels or more.
const NEAR_SCALE: f32 = 0.5;
/// Pixels along a cell's side from near, at most.
const NEAR_PIXELS: u32 = 8;
/// Cells past the view's edges painted from near: what a moving view
/// shows before the next frame comes.
const NEAR_MARGIN: u32 = 8;
/// Superchunks' images kept at most: past that, those out of view go.
const TILES_KEPT: usize = 4096;

/// Superchunks a frame carries at most, drawn at `detail`: fewer the
/// finer, a fine one being more to paint and to send to the graphics
/// card. The window goes round those in view, so many frames.
const fn frame_holds(detail: u32) -> u32 {
    match detail {
        0 | 1 => 8,
        2 => 16,
        _ => 32,
    }
}

/// Dirt, one pixel of it: what a picture is before its first pixels.
pub const DIRT: [u8; 4] = [BROWN[0], BROWN[1], BROWN[2], u8::MAX];

/// An image `size` pixels across and down, of `pixels`.
pub fn picture_of(size: (u32, u32), pixels: Vec<u8>) -> Image {
    let size = Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
    Image::new(size, TextureDimension::D2, pixels, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// What of a picture laid over the superchunks' images is changed:
/// its image, where it lies, whether it shows.
pub type Laid = (&'static Sprite, &'static mut Transform, &'static mut Visibility);

/// The picture of the cells in view from near, over the superchunks' images.
#[derive(Component)]
pub struct NearView;

/// The picture from near, hidden until there is one.
pub fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((Sprite { image: images.add(picture_of((1, 1), DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.0), Visibility::Hidden, NearView));
}

/// Shows the frame the host answered with, if it has: each
/// superchunk's image, and the picture from near -- hidden once the
/// view is no longer near. A frame of another world than the last
/// drops what was drawn of that one, and puts the view over the new.
#[allow(clippy::too_many_arguments)]
pub fn show(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut images: ResMut<Assets<Image>>,
    mut seen: ResMut<Seen>,
    near_view: Single<Laid, (With<NearView>, Without<Camera2d>)>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    window: Single<&Window>,
) {
    let (near_sprite, mut near_transform, mut near_visibility) = near_view.into_inner();
    if seen.near_pixels == 0 {
        *near_visibility = Visibility::Hidden;
    }
    let Some(picture) = link.pictures.lock().expect("the pictures' receiver").try_iter().last() else {
        return;
    };
    link.waiting = false;
    if seen.frame.as_ref().is_none_or(|frame| frame.world != picture.frame.world) {
        for (_, (sprite, ..)) in sprites.tiles.drain() {
            commands.entity(sprite).despawn();
        }
        let (mut transform, mut projection) = camera.into_inner();
        first_view(picture.frame.side, &window, &mut transform, &mut projection);
    }
    (seen.frame, seen.painted, seen.paint_seconds) = (Some(picture.frame), picture.superchunks.len(), picture.paint_seconds);
    for painted in picture.superchunks {
        if painted.cold {
            // Nothing to draw: what was drawn of it goes.
            if let Some((sprite, ..)) = sprites.tiles.remove(&painted.at) {
                commands.entity(sprite).despawn();
            }
            continue;
        }
        let (at, side) = (painted.at, painted.side);
        if let Some((_, image, drawn)) = sprites.tiles.get_mut(&at) {
            if let Some(mut image) = images.get_mut(&*image) {
                *image = picture_of((side, side), painted.pixels);
                *drawn = side;
            }
            continue;
        }
        // Its first pixels: a sprite a superchunk's side on the plane whatever its image's; the world's y grows downwards, the plane's upwards.
        let image = images.add(picture_of((side, side), painted.pixels));
        let sprite = Sprite { image: image.clone(), custom_size: Some(Vec2::splat(SPRITE_SIDE)), ..default() };
        let middle = [0, 1].map(|axis| sprites.plane([at.0, at.1][axis] * SUPERCHUNK_SIDE_CELLS, axis) + SPRITE_SIDE / 2.0);
        let sprite = commands.spawn((sprite, Transform::from_xyz(middle[0], -middle[1], 0.0))).id();
        sprites.tiles.insert(at, (sprite, image, side));
    }
    if let Some(painted) = picture.near {
        let (first, size, pixels) = (painted.near.first, painted.near.size, painted.near.pixels_a_cell);
        if let Some(mut image) = images.get_mut(&near_sprite.image) {
            *image = picture_of((size.0 * pixels, size.1 * pixels), painted.pixels);
        }
        // Over the cells it is of, a cell a unit whatever its pixels.
        let middle = (sprites.plane(first.0, 0) + size.0 as f32 / 2.0, -(sprites.plane(first.1, 1) + size.1 as f32 / 2.0));
        *near_transform = Transform::from_xyz(middle.0, middle.1, 1.0).with_scale(Vec3::new(1.0 / pixels as f32, 1.0 / pixels as f32, 1.0));
        if seen.near_pixels > 0 {
            *near_visibility = Visibility::Visible;
        }
    }
}

/// From near, the cells from `first` to `last` and a margin about
/// them, as many pixels a cell as the screen shows `scale` cells a
/// pixel.
fn near(first: (u32, u32), last: (u32, u32), scale: f32) -> Option<Near> {
    let first = (first.0.saturating_sub(NEAR_MARGIN), first.1.saturating_sub(NEAR_MARGIN));
    let size = (last.0.saturating_add(NEAR_MARGIN) - first.0 + 1, last.1.saturating_add(NEAR_MARGIN) - first.1 + 1);
    let near = Near { first, size, pixels_a_cell: (1 << (1.0 / scale).log2().floor() as u32).min(NEAR_PIXELS) };
    (scale <= NEAR_SCALE && size.0 > 0 && size.1 > 0).then_some(near)
}

/// Asks the host for the next frame: the superchunks now in view.
/// Images of superchunks gone out of view are dropped, but for coarse
/// ones while there are not too many.
pub fn ask(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut seen: ResMut<Seen>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    time: Res<Time>,
) {
    link.since += time.delta_secs();
    if link.waiting || link.since < SYNC_EVERY {
        return;
    }
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let [(left, right), (top, bottom)] = sprites.in_view(transform, view.scale, &window);
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    let viewport = Viewport { first: (superchunk(left), superchunk(top)), last: (superchunk(right), superchunk(bottom)) };
    // A pixel of the screen is `scale` cells: drawn no finer than that.
    let detail = (view.scale.max(1.0).log2().floor() as u32).min(COARSEST);
    let in_view = (viewport.last.0 - viewport.first.0 + 1) * (viewport.last.1 - viewport.first.1 + 1);
    let near = near((left, top), (right, bottom), view.scale);
    // From near every superchunk in view is in the one picture; from as far as the map, none is asked for.
    let most = match near {
        Some(_) => in_view,
        None if view.scale > FARTHEST => 0,
        None => frame_holds(detail),
    };
    // On round the superchunks in view from the last frame's, or from the first if the view changed.
    let skip = match link.asked {
        Some(last) if near.is_none() && last.near.is_none() && last.viewport == viewport && last.detail == detail && last.skip + last.most < in_view => last.skip + last.most,
        _ => 0,
    };
    let ask = Ask { viewport, detail, skip, most, near };
    (seen.in_view, seen.detail, seen.near_pixels) = (in_view, detail, near.map_or(0, |near| near.pixels_a_cell));
    // Fine images of superchunks gone out of view are dropped, each 4 MiB here and as much on the graphics card; and every one out of view, once there are very many.
    let many = sprites.tiles.len() > TILES_KEPT;
    sprites.tiles.retain(|&(x, y), (sprite, _, side)| {
        let in_view = (viewport.first.0..=viewport.last.0).contains(&x) && (viewport.first.1..=viewport.last.1).contains(&y);
        let kept = in_view || (*side <= KEPT_SIDE && !many);
        if !kept {
            commands.entity(*sprite).despawn();
        }
        kept
    });
    link.waiting = link.requests.send(Request::Sync(ask)).is_ok();
    (link.asked, link.since) = (Some(ask), 0.0);
}
