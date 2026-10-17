//! Frames asked of the host ([`ask_next_frame`]) and shown ([`show_frames`]): a
//! superchunk's image a sprite each, and from near one picture of the
//! viewport's cells (`docs/renderer.md`, "The window asks").

use crate::renderer::link::{HostLink, LastFrameSeen};
use crate::renderer::map::MapLink;
use crate::renderer::mipmaps::{picture_with_mipmaps, MipmapsDue};
use crate::renderer::paint::{PaintedFrame, DIRT_BROWN};
use crate::renderer::view::{first_view, SuperchunkSprites, SPRITE_SIDE};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use utilities::coordinates::SUPERCHUNK_SIDE_CELLS;
use protocol::pipe::frame::{FrameAsk, NearRectangle, Viewport};

/// Seconds from one frame asked for to the next, at least: no oftener
/// than a screen shows them.
const SYNC_EVERY: f32 = 1.0 / 60.0;
/// The coarsest the world is drawn: a pixel `2^8` cells a side -- a
/// chunk -- a superchunk 4 pixels.
const COARSEST_DETAIL: u32 = 8;
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

/// Superchunks a frame carries at most, drawn at `detail_shift`: fewer the
/// finer, a fine one being more to paint and to send to the graphics
/// card. The window goes round the viewport's, so many frames.
const fn frame_holds(detail_shift: u32) -> u32 {
    match detail_shift {
        0 | 1 => 8,
        2 => 16,
        _ => 32,
    }
}

/// Dirt, one pixel of it: what a picture is before its first pixels.
pub const DIRT: [u8; 4] = [DIRT_BROWN[0], DIRT_BROWN[1], DIRT_BROWN[2], u8::MAX];

/// An image `size_pixels` pixels across and down, of `rgba_bytes`.
pub fn picture_of(size_pixels: (u32, u32), rgba_bytes: Vec<u8>) -> Image {
    let image_extent = Extent3d { width: size_pixels.0, height: size_pixels.1, depth_or_array_layers: 1 };
    Image::new(image_extent, TextureDimension::D2, rgba_bytes, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// What of a picture laid over the superchunks' images is changed:
/// its image, where it lies, whether it shows.
pub type LaidPictureParts = (&'static Sprite, &'static mut Transform, &'static mut Visibility);

/// The picture of the viewport's cells from near, over the superchunks' images.
#[derive(Component)]
pub struct NearView;

/// The picture from near, hidden until there is one.
pub fn spawn_near_view(mut bevy_commands: Commands, mut images: ResMut<Assets<Image>>) {
    bevy_commands.spawn((Sprite { image: images.add(picture_of((1, 1), DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.0), Visibility::Hidden, NearView));
}

/// Shows the frames the host answered with since the last shown, in
/// the order they came: each superchunk's image, and the picture from
/// near; what went cold or is of another world dropped
/// (`docs/reference.md`, "frames.rs").
#[allow(clippy::too_many_arguments)]
pub fn show_frames(
    mut bevy_commands: Commands,
    mut host_link: ResMut<HostLink>,
    mut superchunk_sprites: ResMut<SuperchunkSprites>,
    mut images: ResMut<Assets<Image>>,
    mut last_seen: ResMut<LastFrameSeen>,
    mut mipmaps_due: ResMut<MipmapsDue>,
    near_view: Single<LaidPictureParts, (With<NearView>, Without<Camera2d>)>,
    camera_parts: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    window: Single<&Window>,
) {
    let (near_sprite, mut near_transform, mut near_visibility) = near_view.into_inner();
    if last_seen.near_pixels == 0 {
        *near_visibility = Visibility::Hidden;
    }
    let new_pictures: Vec<PaintedFrame> = host_link.painted_pictures.lock().expect("the pictures' receiver").try_iter().collect();
    let mut unplaced_camera = Some(camera_parts);
    if !new_pictures.is_empty() {
        last_seen.painted_superchunks = 0;
    }
    for painted_picture in new_pictures {
        host_link.awaiting_frame &= painted_picture.host_frame.more_to_come;
        if last_seen.last_frame.as_ref().is_none_or(|last_frame| last_frame.world != painted_picture.host_frame.world) {
            last_seen.climates.clear();
            for (_, (sprite_entity, ..)) in superchunk_sprites.tiles.drain() {
                bevy_commands.entity(sprite_entity).despawn();
            }
            if std::mem::take(&mut host_link.first_view)
                && let Some(camera_to_place) = unplaced_camera.take()
            {
                let (mut camera_transform, mut camera_projection) = camera_to_place.into_inner();
                first_view(painted_picture.host_frame.side_superchunks, &window, &mut camera_transform, &mut camera_projection);
            }
        }
        if let Some(viewport) = painted_picture.host_frame.viewport {
            // Nothing to draw of a superchunk gone cold: what was drawn of it goes. The hot are row by row.
            let hot = &painted_picture.host_frame.hot;
            let cold: Vec<(u32, u32)> = superchunk_sprites.tiles.keys().copied().filter(|&drawn_superchunk_cartesian| viewport.contains(drawn_superchunk_cartesian) && hot.binary_search_by_key(&(drawn_superchunk_cartesian.1, drawn_superchunk_cartesian.0), |&(x, y)| (y, x)).is_err()).collect();
            for cold_superchunk_cartesian in cold {
                if let Some((cold_sprite_entity, ..)) = superchunk_sprites.tiles.remove(&cold_superchunk_cartesian) {
                    bevy_commands.entity(cold_sprite_entity).despawn();
                }
            }
        }
        last_seen.climates.extend(painted_picture.climates);
        (last_seen.last_frame, last_seen.painted_superchunks, last_seen.paint_seconds) = (Some(painted_picture.host_frame), last_seen.painted_superchunks + painted_picture.superchunks.len(), painted_picture.paint_seconds);
        for painted_superchunk in painted_picture.superchunks {
            let (superchunk_cartesian, side_pixels) = (painted_superchunk.superchunk_cartesian, painted_superchunk.side_pixels);
            // Its mipmaps are the graphics card's to make, each time its pixels change.
            if let Some((_, image, drawn_side_pixels)) = superchunk_sprites.tiles.get_mut(&superchunk_cartesian) {
                if let Some(mut existing_image) = images.get_mut(&*image) {
                    *existing_image = picture_with_mipmaps(side_pixels, painted_superchunk.rgba_bytes);
                    *drawn_side_pixels = side_pixels;
                    mipmaps_due.0.push(image.id());
                }
                continue;
            }
            // Its first pixels: a sprite a superchunk's side on the plane whatever its image's; the world's y grows downwards, the plane's upwards.
            let image = images.add(picture_with_mipmaps(side_pixels, painted_superchunk.rgba_bytes));
            mipmaps_due.0.push(image.id());
            let new_sprite = Sprite { image: image.clone(), custom_size: Some(Vec2::splat(SPRITE_SIDE)), ..default() };
            let sprite_middle = [0, 1].map(|cartesian_axis| superchunk_sprites.translation_of_cell([superchunk_cartesian.0, superchunk_cartesian.1][cartesian_axis] * SUPERCHUNK_SIDE_CELLS, cartesian_axis) + SPRITE_SIDE / 2.0);
            let spawned_sprite_entity = bevy_commands.spawn((new_sprite, Transform::from_xyz(sprite_middle[0], -sprite_middle[1], 0.0))).id();
            superchunk_sprites.tiles.insert(superchunk_cartesian, (spawned_sprite_entity, image, side_pixels));
        }
        if let Some(painted_near) = painted_picture.painted_near {
            let (first_cell_cartesian, size_cells, pixels_a_cell) = (painted_near.near_asked.first_cell_cartesian, painted_near.near_asked.size_in_cells, painted_near.near_asked.pixels_a_cell);
            if let Some(mut image) = images.get_mut(&near_sprite.image) {
                *image = picture_of((size_cells.0 * pixels_a_cell, size_cells.1 * pixels_a_cell), painted_near.rgba_bytes);
            }
            // Over the cells it is of, a cell a unit whatever its pixels.
            let near_middle = (superchunk_sprites.translation_of_cell(first_cell_cartesian.0, 0) + size_cells.0 as f32 / 2.0, -(superchunk_sprites.translation_of_cell(first_cell_cartesian.1, 1) + size_cells.1 as f32 / 2.0));
            *near_transform = Transform::from_xyz(near_middle.0, near_middle.1, 1.0).with_scale(Vec3::new(1.0 / pixels_a_cell as f32, 1.0 / pixels_a_cell as f32, 1.0));
            if last_seen.near_pixels > 0 {
                *near_visibility = Visibility::Visible;
            }
        }
    }
}

/// From near, the cells from `first_cell_cartesian` to `last_cell_cartesian` and a margin about
/// them, as many pixels a cell as the screen shows `scale` cells a
/// pixel.
fn near_of_cells(first_cell_cartesian: (u32, u32), last_cell_cartesian: (u32, u32), scale: f32) -> Option<NearRectangle> {
    let margined_first = (first_cell_cartesian.0.saturating_sub(NEAR_MARGIN), first_cell_cartesian.1.saturating_sub(NEAR_MARGIN));
    let margined_size = (last_cell_cartesian.0.saturating_add(NEAR_MARGIN) - margined_first.0 + 1, last_cell_cartesian.1.saturating_add(NEAR_MARGIN) - margined_first.1 + 1);
    let near_wanted = NearRectangle { first_cell_cartesian: margined_first, size_in_cells: margined_size, pixels_a_cell: (1 << (1.0 / scale).log2().floor() as u32).min(NEAR_PIXELS) };
    (scale <= NEAR_SCALE && margined_size.0 > 0 && margined_size.1 > 0).then_some(near_wanted)
}

/// How coarsely the world is drawn, the screen showing `scale` cells a
/// pixel: a pixel `2^detail` cells a side. Near, no coarser than the
/// screen; past [`COARSER_FROM`] halvings, coarser by as many again --
/// far out, a superchunk is a few pixels however many there are.
pub fn detail_at(scale: f32) -> u32 {
    let scale_halvings = scale.max(1.0).log2().floor() as u32;
    (scale_halvings + scale_halvings.saturating_sub(COARSER_FROM)).min(COARSEST_DETAIL)
}

/// Asks the host for the next frame: the hot superchunks of the
/// viewport -- none in map mode. Images of superchunks gone out of the
/// viewport are dropped, but for coarse ones while there are not too
/// many.
#[allow(clippy::too_many_arguments)]
pub fn ask_next_frame(
    mut bevy_commands: Commands,
    mut host_link: ResMut<HostLink>,
    mut superchunk_sprites: ResMut<SuperchunkSprites>,
    mut last_seen: ResMut<LastFrameSeen>,
    map: Res<MapLink>,
    camera_parts: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    frame_time: Res<Time>,
) {
    host_link.seconds_since_asked += frame_time.delta_secs();
    if host_link.awaiting_frame || host_link.seconds_since_asked < SYNC_EVERY {
        return;
    }
    let (camera_transform, camera_projection) = *camera_parts;
    let Projection::Orthographic(orthographic_view) = camera_projection else {
        return;
    };
    let [(left_cell_column, right_cell_column), (top_cell_row, bottom_cell_row)] = superchunk_sprites.viewport_cells(camera_transform, orthographic_view.scale, &window);
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    // In map mode the map is rendered, none of the cells.
    let viewport = (!map.map_mode()).then(|| Viewport { top_left_superchunk_cartesian: (superchunk(left_cell_column), superchunk(top_cell_row)), bottom_right_superchunk_cartesian: (superchunk(right_cell_column), superchunk(bottom_cell_row)) });
    let detail_shift = detail_at(orthographic_view.scale);
    let viewport_superchunks = viewport.map_or(0, |viewport| (viewport.bottom_right_superchunk_cartesian.0 - viewport.top_left_superchunk_cartesian.0 + 1) * (viewport.bottom_right_superchunk_cartesian.1 - viewport.top_left_superchunk_cartesian.1 + 1));
    let near_wanted = viewport.and_then(|_| near_of_cells((left_cell_column, top_cell_row), (right_cell_column, bottom_cell_row), orthographic_view.scale));
    // From near every hot superchunk of the viewport is in the one picture; in map mode, none is asked for.
    let most_superchunks = match (viewport, near_wanted) {
        (None, _) => 0,
        (Some(_), Some(_)) => u32::MAX,
        (Some(_), None) => frame_holds(detail_shift),
    };
    // On round the viewport's hot superchunks from the last frame's, or from the first if anything changed.
    let hot = last_seen.last_frame.as_ref().map_or(0, |last_frame| last_frame.hot.len() as u32);
    let skipped_superchunks = match host_link.last_ask {
        Some(last_ask) if near_wanted.is_none() && last_ask.seen_near.is_none() && last_ask.viewport == viewport && last_ask.pixel_side_log2 == detail_shift && last_ask.superchunks_skipped.saturating_add(last_ask.most_superchunks) < hot => last_ask.superchunks_skipped + last_ask.most_superchunks,
        _ => 0,
    };
    let next_ask = FrameAsk { viewport, pixel_side_log2: detail_shift, superchunks_skipped: skipped_superchunks, most_superchunks, seen_near: near_wanted };
    (last_seen.viewport_superchunks, last_seen.detail_shift, last_seen.near_pixels) = (viewport_superchunks, detail_shift, near_wanted.map_or(0, |asked_near| asked_near.pixels_a_cell));
    // Fine images of superchunks gone out of the viewport are dropped, each 4 MiB here and as much on the graphics card; and every one out of it, once there are very many.
    let too_many_kept = superchunk_sprites.tiles.len() > TILES_KEPT;
    superchunk_sprites.tiles.retain(|&superchunk_cartesian, (sprite_entity, _, side_pixels)| {
        let in_viewport = viewport.is_some_and(|viewport| viewport.contains(superchunk_cartesian));
        let is_kept = in_viewport || (*side_pixels <= KEPT_SIDE && !too_many_kept);
        if !is_kept {
            bevy_commands.entity(*sprite_entity).despawn();
        }
        is_kept
    });
    host_link.awaiting_frame = host_link.server().ask_frame(next_ask);
    (host_link.last_ask, host_link.seconds_since_asked) = (Some(next_ask), 0.0);
}
