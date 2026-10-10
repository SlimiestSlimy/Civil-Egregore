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

/// A superchunk's picture `side` pixels a side -- a power of two -- of
/// `pixels`, with room for its mipmaps, which the graphics card makes
/// ([`MipmapsDue`]): from near each pixel sharp, from far the mipmaps
/// blended.
pub fn picture_with_mipmaps(side: u32, pixels: Vec<u8>) -> Image {
    debug_assert!(side.is_power_of_two(), "a picture {side} pixels a side has no halvings down to one");
    let size = Extent3d { width: side, height: side, depth_or_array_layers: 1 };
    // Kept as the numbers painted, so the card may write its halvings; read as the colours they are.
    let mut image = Image::new(size, TextureDimension::D2, pixels, TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
    let levels = side.ilog2() + 1;
    image.texture_descriptor.mip_level_count = levels;
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    image.texture_descriptor.view_formats = &[TextureFormat::Rgba8UnormSrgb];
    // The view it is drawn through only reads: colours cannot be written as they are read.
    image.texture_view_descriptor = Some(TextureViewDescriptor { format: Some(TextureFormat::Rgba8UnormSrgb), usage: Some(TextureUsages::TEXTURE_BINDING), ..default() });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { mag_filter: ImageFilterMode::Nearest, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, ..default() });
    // A new picture is sent whole, its mipmaps' room with it: empty until the card fills it.
    let whole: usize = (0..levels).map(|level| ((side >> level) as usize).pow(2) * 4).sum();
    if let Some(pixels) = &mut image.data {
        pixels.resize(whole, 0);
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
struct Waiting {
    /// Each picture waiting, and for how many frames.
    pictures: HashMap<AssetId<Image>, u32>,
    /// Frames mipmaps were asked on, so far.
    asked_on: u32,
}

/// Begins a frame: no picture's mipmaps due yet -- but, the first
/// time, those of a small picture, so what makes them is built.
fn begin(mut commands: Commands, mut due: ResMut<MipmapsDue>, mut images: ResMut<Assets<Image>>, warm_up: Option<Res<WarmUp>>) {
    due.0.clear();
    if warm_up.is_none() {
        let picture = images.add(picture_with_mipmaps(WARM_UP_SIDE, vec![0; (WARM_UP_SIDE * WARM_UP_SIDE * 4) as usize]));
        due.0.push(picture.id());
        commands.insert_resource(WarmUp(picture));
    }
}

/// Takes the frame's pictures due to the card's side, and asks for the
/// mipmaps of every one still waiting.
fn take(due: Extract<Res<MipmapsDue>>, mut waiting: ResMut<Waiting>, mut jobs: ResMut<MipGenerationJobs>) {
    for &picture in &due.0 {
        waiting.pictures.insert(picture, 0);
    }
    for &picture in waiting.pictures.keys() {
        jobs.add(BEFORE_THE_CAMERAS, picture);
    }
    waiting.asked_on = waiting.asked_on.saturating_add(u32::from(!waiting.pictures.is_empty()));
}

/// Makes the mipmaps asked for, before the cameras draw; a picture
/// waits on until the card has it and what makes mipmaps is built.
fn make(jobs: Res<MipGenerationJobs>, cache: Res<PipelineCache>, pipelines: Res<MipGenerationPipelines>, images: Res<RenderAssets<GpuImage>>, mut waiting: ResMut<Waiting>, mut context: RenderContext) {
    generate_mips_for_phase(BEFORE_THE_CAMERAS, &jobs, &cache, &pipelines, &images, &mut context);
    let built = waiting.asked_on >= ASKED_BEFORE && cache.waiting_pipelines().next().is_none();
    waiting.pictures.retain(|&picture, frames| {
        *frames += 1;
        let made = built && images.get(picture).is_some();
        !made && *frames < GIVEN_UP_AFTER
    });
}

/// The pictures' mipmaps made by the graphics card.
pub struct Mipmaps;

impl Plugin for Mipmaps {
    fn build(&self, app: &mut App) {
        app.init_resource::<MipmapsDue>().add_systems(First, begin);
        let Some(card) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        card.init_resource::<Waiting>().add_systems(ExtractSchedule, take).add_systems(RenderGraph, make.before(camera_driver));
    }
}
