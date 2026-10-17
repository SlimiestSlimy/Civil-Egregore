//! A frame made of the world run: the hot superchunks a client asked
//! for, each copied as a tick left it and nothing more
//! (`protocol::pipe::frame`; `docs/server.md`, "The host").

use crate::World;
use bitplane_manager::BucketKey;
use base_types::{CLIMATE, CLOUD, GRASS, TREE, TREE_STAGE, WET};
use chunk_storage::{LayerType, SuperchunkImage};
use utilities::coordinates::{CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use base_types::plane_words::raining_tiles_of;
use protocol::pipe::frame::{Cells, FrameAsk, Viewport, ViewportFrame, Water, CHUNK_WORDS, DEEP_WATER_DEPTH};
use super::terrain::drawn_heights_of;
use std::collections::HashSet;

/// The hot superchunks of `world` in `viewport`, each `(x, y)` in the
/// world, row by row: none if there is no viewport. Hot superchunks are
/// few, however wide the viewport.
pub(crate) fn hot_in(world: &World, viewport: Option<Viewport>) -> Vec<(u32, u32)> {
    let Some(viewport) = viewport else {
        return Vec::new();
    };
    let mut hot: Vec<(u32, u32)> = world.entity_store().superchunks().iter().map(|superchunk_entities| superchunk_entities.superchunk_index().cartesian()).filter(|&superchunk_cartesian| viewport.contains(superchunk_cartesian)).collect();
    hot.sort_unstable_by_key(|&(x, y)| (y, x));
    hot
}

/// A frame of `world`, the `worlds`-th a client is shown: what `frame_ask`
/// asked, `hot` the viewport's hot superchunks, `cells` those copied
/// for it; `measured` the ticks a second, the seconds the copy took
/// and the share of the time that is; the world's name and what was
/// last said of it; and whether `more_to_come` of what was asked is to come.
/// The one making of a frame: the host's thread's and a shown run's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_of(world: &World, worlds: u64, frame_ask: FrameAsk, hot: Vec<(u32, u32)>, cells: Vec<Cells>, (ticks_a_second, sync_seconds, sync_share): (f64, f64, f64), (world_name, last_outcome): (Option<String>, Option<String>), more_to_come: bool) -> ViewportFrame {
    let (sheep_count, grass_cells, trees) = (world.entity_store().len(), count(world, GRASS), count(world, TREE));
    ViewportFrame { world: worlds, seed: world.world_info.seed, generation_numbers: world.world_generation.numbers(), drawn_heights: drawn_heights_of(&world.world_generation), side_superchunks: world.halos().hot.world_side_superchunks(), tick: world.entity_store().current_tick(), ticks_a_second, sheep_count, grass_cells, trees, sync_seconds, sync_share, viewport: frame_ask.viewport, hot, pixel_side_log2: frame_ask.pixel_side_log2, seen_near: frame_ask.seen_near, world_name, last_outcome, more_to_come, cells }
}

/// Cells of `layer_type` over every hot superchunk of `world`.
pub(crate) fn count(world: &World, layer_type: LayerType) -> u64 {
    world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(layer_type, superchunk) as u64).sum()
}

/// The superchunk at `(x, y)` of `world` copied as `frame_ask` asks, if it
/// is hot still: its planes, words as they are, and its sheep's cells
/// -- its heights and water only the first time it is copied,
/// `heights_sent` keeping which.
pub(crate) fn copy_superchunk(world: &World, (x, y): (u32, u32), frame_ask: FrameAsk, heights_sent: &mut HashSet<SuperchunkIndex>) -> Option<Cells> {
    let superchunk = SuperchunkIndex::from_cartesian(x, y);
    world.entity_store().superchunk(superchunk)?;
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    let plane_words = |layer_type: LayerType| layer(world, layer_type, superchunk);
    // Heights never change: sent the once, in the first frame the superchunk is hot in. Nor does the water's depth.
    let sent_first = heights_sent.insert(superchunk);
    let image = world.chunk_storage().image(superchunk).filter(|_| sent_first);
    let heights = image.map_or(Vec::new(), |image| image.height_words().to_vec());
    let water = sent_first.then(|| water_of(image));
    // A tree's stage is drawn only where a cell is a pixel or more.
    let stages = if frame_ask.seen_near.is_some() || frame_ask.pixel_side_log2 == 0 { plane_words(TREE_STAGE.layer_type()) } else { Vec::new() };
    // No cloud over it: nothing to send, nothing to draw.
    let clouds = Some(plane_words(CLOUD.layer_type())).filter(|clouds| clouds.iter().any(|&word| word != 0)).unwrap_or_default();
    // Which of its tiles rain is the climate's to say, of each tile's own: worked out here, so a client need not.
    let climates = plane_words(CLIMATE.layer_type());
    let raining_tiles = if clouds.is_empty() { Vec::new() } else { raining_tiles_of(&clouds, &climates) };
    Some(Cells { superchunk_cartesian: (x, y), heights, top_left: (left_cell_x, top_cell_y), grass_bitmaps: plane_words(GRASS), trees: plane_words(TREE), stages, wet_bitmaps: plane_words(WET), clouds, raining_tiles, climates, water, sheep_cells_cartesian: sheep_cells_of(world, superchunk) })
}

/// The water of `image`, as a client draws it: none of none, and of
/// one with no water.
fn water_of(image: Option<&SuperchunkImage>) -> Water {
    let words = CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS;
    let mut water = Water { depth_low_bits: std::array::from_fn(|_| vec![0; words]), deep_cells: vec![0; words] };
    let Some(image) = image else {
        return water;
    };
    for place in 0..words * u64::BITS as usize {
        let (cell_depth, word, bit_in_word) = (image.water_depth(place) as u32, place / u64::BITS as usize, place % u64::BITS as usize);
        if cell_depth >= DEEP_WATER_DEPTH {
            water.deep_cells[word] |= 1 << bit_in_word;
            continue;
        }
        for (depth_bit, depth_bitmap) in water.depth_low_bits.iter_mut().enumerate() {
            depth_bitmap[word] |= ((cell_depth >> depth_bit & 1) as u64) << bit_in_word;
        }
    }
    water
}

/// `superchunk`'s cells of `layer_type`: its chunks' words, one chunk
/// after another -- of a wide type, as many times a bitmap's words a
/// chunk as it has bits a cell.
fn layer(world: &World, layer_type: LayerType, superchunk: SuperchunkIndex) -> Vec<u64> {
    let chunk_words = chunk_storage::wide::bucket_words(layer_type);
    let mut words = Vec::with_capacity(CHUNKS_IN_SUPERCHUNK * chunk_words);
    for chunk in superchunk.chunks() {
        match world.bitmap_arena().bucket(BucketKey { layer_type, chunk }) {
            Some(bucket) => words.extend_from_slice(bucket.words()),
            None => words.resize(words.len() + chunk_words, 0),
        }
    }
    words
}

/// The cells `superchunk`'s sheep stand on, from its top left.
fn sheep_cells_of(world: &World, superchunk: SuperchunkIndex) -> Vec<(u16, u16)> {
    let Some(superchunk_entities) = world.entity_store().superchunk(superchunk) else {
        return Vec::new();
    };
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    superchunk_entities.iter().map(|entity| entity.header.stood_cell.cartesian()).map(|sheep_cell| ((sheep_cell.x - left_cell_x) as u16, (sheep_cell.y - top_cell_y) as u16)).collect()
}
