//! Mipmaps of the superchunks' pictures, made by the graphics card: a
//! picture is sent with room for every halving of it, and the card
//! fills them in whenever its pixels change (`docs/renderer.md`,
//! "Mipmaps made on the graphics card").

use bevy::asset::{AssetId, RenderAssetUsages};
use bevy::core_pipeline::mip_generation::{generate_mips_for_phase, MipGenerationJobs, MipGenerationPhaseId, MipGenerationPipelines};
use bevy::core_pipeline::schedule::camera_driver;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{Extent3d, PipelineCache, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use bevy::render::renderer::{RenderContext, RenderGraph};
use bevy::render::texture::GpuImage;
use bevy::render::{Extract, ExtractSchedule, RenderApp};
use std::collections::HashMap;

/// The phase the pictures' mipmaps are made in: before any camera
/// draws.
const BEFORE_THE_CAMERAS: MipGenerationPhaseId = MipGenerationPhaseId(0);
/// Frames a picture's mipmaps are waited for before they are given up:
/// its image gone before the card had it.
const GIVEN_UP_AFTER: u32 = 600;
/// Frames mipmaps must have been asked on before any is taken as made:
/// on the first, what makes them is only being built.
const ASKED_BEFORE: u32 = 2;
/// Pixels along the side of the picture the card's mipmap maker is
/// built for, before a world is shown.
const WARM_UP_SIDE: u32 = 4;

/// A superchunk's picture `side_pixels` pixels a side -- a power of two -- of
/// `rgba_bytes`, with room for its mipmaps, which the graphics card makes
/// ([`MipmapsDue`]): from near each pixel sharp, from far the mipmaps
/// blended.
pub fn picture_with_mipmaps(side_pixels: u32, rgba_bytes: Vec<u8>) -> Image {
    debug_assert!(side_pixels.is_power_of_two(), "a picture {side_pixels} pixels a side has no halvings down to one");
    let image_extent = Extent3d { width: side_pixels, height: side_pixels, depth_or_array_layers: 1 };
    // Kept as the numbers painted, so the card may write its halvings; read as the colours they are.
    let mut image = Image::new(image_extent, TextureDimension::D2, rgba_bytes, TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
    let mipmap_count = side_pixels.ilog2() + 1;
    image.texture_descriptor.mip_level_count = mipmap_count;
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    image.texture_descriptor.view_formats = &[TextureFormat::Rgba8UnormSrgb];
    // The view it is drawn through only reads: colours cannot be written as they are read.
    image.texture_view_descriptor = Some(TextureViewDescriptor { format: Some(TextureFormat::Rgba8UnormSrgb), usage: Some(TextureUsages::TEXTURE_BINDING), ..default() });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { mag_filter: ImageFilterMode::Nearest, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, ..default() });
    // A new picture is sent whole, its mipmaps' room with it: empty until the card fills it.
    let whole_bytes: usize = (0..mipmap_count).map(|mipmap_number| ((side_pixels >> mipmap_number) as usize).pow(2) * 4).sum();
    if let Some(image_bytes) = &mut image.data {
        image_bytes.resize(whole_bytes, 0);
    }
    image
}

/// The pictures whose pixels changed this frame: their mipmaps are to
/// be made again. Whoever changes a picture made by
/// [`picture_with_mipmaps`] names it here.
#[derive(Resource, Default)]
pub struct MipmapsDue(pub Vec<AssetId<Image>>);

/// The picture the card's mipmap maker is first built for, kept so it
/// is built before a world is shown.
#[derive(Resource)]
struct WarmUp(#[allow(dead_code, reason = "held so the picture stays")] Handle<Image>);

/// On the graphics card's side: the pictures whose mipmaps are not
/// made yet, each with the frames it has waited, and the frames any
/// was asked on.
#[derive(Resource, Default)]
struct MipmapsWaiting {
    /// Each picture waiting, and for how many frames.
    waiting_pictures: HashMap<AssetId<Image>, u32>,
    /// Frames mipmaps were asked on, so far.
    asked_on: u32,
}

/// Begins a frame: no picture's mipmaps due yet -- but, the first
/// time, those of a small picture, so what makes them is built.
fn begin_frame_mipmaps(mut bevy_commands: Commands, mut mipmaps_due: ResMut<MipmapsDue>, mut images: ResMut<Assets<Image>>, warm_up: Option<Res<WarmUp>>) {
    mipmaps_due.0.clear();
    if warm_up.is_none() {
        let warm_up_picture = images.add(picture_with_mipmaps(WARM_UP_SIDE, vec![0; (WARM_UP_SIDE * WARM_UP_SIDE * 4) as usize]));
        mipmaps_due.0.push(warm_up_picture.id());
        bevy_commands.insert_resource(WarmUp(warm_up_picture));
    }
}

/// Takes the frame's pictures due to the card's side, and asks for the
/// mipmaps of every one still waiting.
fn take_due_pictures(mipmaps_due: Extract<Res<MipmapsDue>>, mut mipmaps_waiting: ResMut<MipmapsWaiting>, mut jobs: ResMut<MipGenerationJobs>) {
    for &due_picture in &mipmaps_due.0 {
        mipmaps_waiting.waiting_pictures.insert(due_picture, 0);
    }
    for &waiting_picture in mipmaps_waiting.waiting_pictures.keys() {
        jobs.add(BEFORE_THE_CAMERAS, waiting_picture);
    }
    mipmaps_waiting.asked_on = mipmaps_waiting.asked_on.saturating_add(u32::from(!mipmaps_waiting.waiting_pictures.is_empty()));
}

/// Makes the mipmaps asked for, before the cameras draw; a picture
/// waits on until the card has it and what makes mipmaps is built.
fn make_mipmaps(jobs: Res<MipGenerationJobs>, pipeline_cache: Res<PipelineCache>, mip_pipelines: Res<MipGenerationPipelines>, images: Res<RenderAssets<GpuImage>>, mut mipmaps_waiting: ResMut<MipmapsWaiting>, mut render_context: RenderContext) {
    generate_mips_for_phase(BEFORE_THE_CAMERAS, &jobs, &pipeline_cache, &mip_pipelines, &images, &mut render_context);
    let maker_built = mipmaps_waiting.asked_on >= ASKED_BEFORE && pipeline_cache.waiting_pipelines().next().is_none();
    mipmaps_waiting.waiting_pictures.retain(|&waiting_picture, frames_waited| {
        *frames_waited += 1;
        let mipmaps_made = maker_built && images.get(waiting_picture).is_some();
        !mipmaps_made && *frames_waited < GIVEN_UP_AFTER
    });
}

/// The pictures' mipmaps made by the graphics card.
pub struct Mipmaps;

impl Plugin for Mipmaps {
    fn build(&self, window_app: &mut App) {
        window_app.init_resource::<MipmapsDue>().add_systems(First, begin_frame_mipmaps);
        let Some(render_app) = window_app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<MipmapsWaiting>().add_systems(ExtractSchedule, take_due_pictures).add_systems(RenderGraph, make_mipmaps.before(camera_driver));
    }
}
