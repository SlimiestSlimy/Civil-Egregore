//! Frames asked of the host ([`ask`]) and shown ([`show`]): a
//! superchunk's image a sprite each, and from near one picture of the
//! viewport's cells (`docs/renderer.md`, "The window asks").

use crate::link::{Link, Seen};
use crate::map::MapLink;
use crate::mipmaps::{picture_with_mipmaps, MipmapsDue};
use crate::paint::{Picture, BROWN};
use crate::view::{first_view, Sprites, SPRITE_SIDE};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use coordinates::SUPERCHUNK_SIDE_CELLS;
use server::host::frame::{Ask, Near, Viewport};

/// Seconds from one frame asked for to the next, at least: no oftener
/// than a screen shows them.
const SYNC_EVERY: f32 = 1.0 / 60.0;
/// The coarsest the world is drawn: a pixel `2^8` cells a side -- a
/// chunk -- a superchunk 4 pixels.
const COARSEST: u32 = 8;
/// Halvings of a pixel of the screen past which the drawing is coarser
/// than the screen: from 4 cells a pixel on, each halving one more.
const COARSER_FROM: u32 = 2;
/// Pixels along the side of an image kept when its superchunk leaves
/// the viewport, at most: a finer one is dropped, to be asked for again.
const KEPT_SIDE: u32 = 64;
/// Cells a screen pixel at most for the view to be from near: a cell
/// two pixels or more.
const NEAR_SCALE: f32 = 0.5;
/// Pixels along a cell's side from near, at most.
const NEAR_PIXELS: u32 = 8;
/// Cells past the view's edges painted from near: what a moving view
/// shows before the next frame comes.
const NEAR_MARGIN: u32 = 8;
/// Superchunks' images kept at most: past that, those out of the
/// viewport go.
const TILES_KEPT: usize = 4096;

/// Superchunks a frame carries at most, drawn at `detail`: fewer the
/// finer, a fine one being more to paint and to send to the graphics
/// card. The window goes round the viewport's, so many frames.
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

/// The picture of the viewport's cells from near, over the superchunks' images.
#[derive(Component)]
pub struct NearView;

/// The picture from near, hidden until there is one.
pub fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((Sprite { image: images.add(picture_of((1, 1), DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.0), Visibility::Hidden, NearView));
}

/// Shows the frames the host answered with since the last shown, in
/// the order they came: each superchunk's image, and the picture from
/// near; what went cold or is of another world dropped
/// (`docs/reference.md`, "frames.rs").
#[allow(clippy::too_many_arguments)]
pub fn show(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut images: ResMut<Assets<Image>>,
    mut seen: ResMut<Seen>,
    mut due: ResMut<MipmapsDue>,
    near_view: Single<Laid, (With<NearView>, Without<Camera2d>)>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    window: Single<&Window>,
) {
    let (near_sprite, mut near_transform, mut near_visibility) = near_view.into_inner();
    if seen.near_pixels == 0 {
        *near_visibility = Visibility::Hidden;
    }
    let pictures: Vec<Picture> = link.pictures.lock().expect("the pictures' receiver").try_iter().collect();
    let mut camera = Some(camera);
    if !pictures.is_empty() {
        seen.painted = 0;
    }
    for picture in pictures {
        link.waiting &= picture.frame.more;
        if seen.frame.as_ref().is_none_or(|frame| frame.world != picture.frame.world) {
            for (_, (sprite, ..)) in sprites.tiles.drain() {
                commands.entity(sprite).despawn();
            }
            if std::mem::take(&mut link.first_view)
                && let Some(camera) = camera.take()
            {
                let (mut transform, mut projection) = camera.into_inner();
                first_view(picture.frame.side, &window, &mut transform, &mut projection);
            }
        }
        if let Some(viewport) = picture.frame.viewport {
            // Nothing to draw of a superchunk gone cold: what was drawn of it goes. The hot are row by row.
            let hot = &picture.frame.hot;
            let cold: Vec<(u32, u32)> = sprites.tiles.keys().copied().filter(|&at| viewport.contains(at) && hot.binary_search_by_key(&(at.1, at.0), |&(x, y)| (y, x)).is_err()).collect();
            for at in cold {
                if let Some((sprite, ..)) = sprites.tiles.remove(&at) {
                    commands.entity(sprite).despawn();
                }
            }
        }
        (seen.frame, seen.painted, seen.paint_seconds) = (Some(picture.frame), seen.painted + picture.superchunks.len(), picture.paint_seconds);
        for painted in picture.superchunks {
            let (at, side) = (painted.at, painted.side);
            // Its mipmaps are the graphics card's to make, each time its pixels change.
            if let Some((_, image, drawn)) = sprites.tiles.get_mut(&at) {
                if let Some(mut picture) = images.get_mut(&*image) {
                    *picture = picture_with_mipmaps(side, painted.pixels);
                    *drawn = side;
                    due.0.push(image.id());
                }
                continue;
            }
            // Its first pixels: a sprite a superchunk's side on the plane whatever its image's; the world's y grows downwards, the plane's upwards.
            let image = images.add(picture_with_mipmaps(side, painted.pixels));
            due.0.push(image.id());
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

/// How coarsely the world is drawn, the screen showing `scale` cells a
/// pixel: a pixel `2^detail` cells a side. Near, no coarser than the
/// screen; past [`COARSER_FROM`] halvings, coarser by as many again --
/// far out, a superchunk is a few pixels however many there are.
pub fn detail_at(scale: f32) -> u32 {
    let halvings = scale.max(1.0).log2().floor() as u32;
    (halvings + halvings.saturating_sub(COARSER_FROM)).min(COARSEST)
}

/// Asks the host for the next frame: the hot superchunks of the
/// viewport -- none in map mode. Images of superchunks gone out of the
/// viewport are dropped, but for coarse ones while there are not too
/// many.
#[allow(clippy::too_many_arguments)]
pub fn ask(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut seen: ResMut<Seen>,
    map: Res<MapLink>,
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
    let [(left, right), (top, bottom)] = sprites.viewport_cells(transform, view.scale, &window);
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    // In map mode the map is rendered, none of the cells.
    let viewport = (!map.map_mode()).then(|| Viewport { first: (superchunk(left), superchunk(top)), last: (superchunk(right), superchunk(bottom)) });
    let detail = detail_at(view.scale);
    let viewport_superchunks = viewport.map_or(0, |viewport| (viewport.last.0 - viewport.first.0 + 1) * (viewport.last.1 - viewport.first.1 + 1));
    let near = viewport.and_then(|_| near((left, top), (right, bottom), view.scale));
    // From near every hot superchunk of the viewport is in the one picture; in map mode, none is asked for.
    let most = match (viewport, near) {
        (None, _) => 0,
        (Some(_), Some(_)) => u32::MAX,
        (Some(_), None) => frame_holds(detail),
    };
    // On round the viewport's hot superchunks from the last frame's, or from the first if anything changed.
    let hot = seen.frame.as_ref().map_or(0, |frame| frame.hot.len() as u32);
    let skip = match link.asked {
        Some(last) if near.is_none() && last.near.is_none() && last.viewport == viewport && last.detail == detail && last.skip.saturating_add(last.most) < hot => last.skip + last.most,
        _ => 0,
    };
    let ask = Ask { viewport, detail, skip, most, near };
    (seen.viewport_superchunks, seen.detail, seen.near_pixels) = (viewport_superchunks, detail, near.map_or(0, |near| near.pixels_a_cell));
    // Fine images of superchunks gone out of the viewport are dropped, each 4 MiB here and as much on the graphics card; and every one out of it, once there are very many.
    let many = sprites.tiles.len() > TILES_KEPT;
    sprites.tiles.retain(|&at, (sprite, _, side)| {
        let in_viewport = viewport.is_some_and(|viewport| viewport.contains(at));
        let kept = in_viewport || (*side <= KEPT_SIDE && !many);
        if !kept {
            commands.entity(*sprite).despawn();
        }
        kept
    });
    link.waiting = link.host.sync(ask);
    (link.asked, link.since) = (Some(ask), 0.0);
}
