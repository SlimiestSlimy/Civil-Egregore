//! A frame made of the world run: the hot superchunks a client asked
//! for, each copied as a tick left it and nothing more -- the planes it
//! asked for, whatever they are (`protocol::pipe::frame`;
//! `docs/server.md`, "The host").

use crate::World;
use crate::bitplane_manager::BucketKey;
use crate::chunk_storage::LayerType;
use utilities::coordinates::{CellCartesian, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use protocol::pipe::frame::{Cells, FrameAsk, Viewport, ViewportFrame};

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
pub(crate) fn frame_of(world: &World, worlds: u64, frame_ask: &FrameAsk, hot: Vec<(u32, u32)>, cells: Vec<Cells>, (ticks_a_second, sync_seconds, sync_share): (f64, f64, f64), held_pace: Option<u32>, (world_name, last_outcome): (Option<String>, Option<String>), more_to_come: bool) -> ViewportFrame {
    let counts = frame_ask.counted.iter().map(|&layer_type| (layer_type, count(world, layer_type))).collect();
    ViewportFrame { world: worlds, seed: world.world_info.seed, worldgen_parameters: world.world_info.worldgen_parameters.clone(), side_superchunks: world.halos().hot.world_side_superchunks(), tick: world.entity_store().current_tick(), ticks_a_second, target_pace: super::TARGET_PACE, held_pace, entity_count: world.entity_store().len(), counts, sync_seconds, sync_share, viewport: frame_ask.viewport, hot, pixel_side_log2: frame_ask.pixel_side_log2, seen_near: frame_ask.seen_near, world_name, last_outcome, more_to_come, cells }
}

/// Cells of `layer_type` over every hot superchunk of `world`.
pub(crate) fn count(world: &World, layer_type: LayerType) -> u64 {
    world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(layer_type, superchunk) as u64).sum()
}

/// The superchunk at `(x, y)` of `world` copied as `frame_ask` asks, if it
/// is hot still: the planes asked for, words as they are -- one all
/// clear sent empty -- and its entities' cells.
pub(crate) fn copy_superchunk(world: &World, (x, y): (u32, u32), frame_ask: &FrameAsk) -> Option<Cells> {
    let superchunk = SuperchunkIndex::from_cartesian(x, y);
    world.entity_store().superchunk(superchunk)?;
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    // A plane all clear: nothing to send, nothing to draw.
    let planes = frame_ask.planes.iter().map(|&layer_type| (layer_type, Some(layer(world, layer_type, superchunk)).filter(|words| words.iter().any(|&word| word != 0)).unwrap_or_default())).collect();
    Some(Cells { superchunk_cartesian: (x, y), planes, top_left: (left_cell_x, top_cell_y), entity_cells_cartesian: entity_cells_of(world, superchunk) })
}

/// `superchunk`'s cells of `layer_type`: its chunks' words, one chunk
/// after another -- of a wide type, as many times a bitmap's words a
/// chunk as it has bits a cell.
fn layer(world: &World, layer_type: LayerType, superchunk: SuperchunkIndex) -> Vec<u64> {
    let chunk_words = crate::chunk_storage::wide::bucket_words(layer_type);
    let mut words = Vec::with_capacity(CHUNKS_IN_SUPERCHUNK * chunk_words);
    for chunk in superchunk.chunks() {
        match world.bitmap_arena().bucket(BucketKey { layer_type, chunk }) {
            Some(bucket) => words.extend_from_slice(bucket.words()),
            None => words.resize(words.len() + chunk_words, 0),
        }
    }
    words
}

/// The cells `superchunk`'s entities stand on, from its top left.
fn entity_cells_of(world: &World, superchunk: SuperchunkIndex) -> Vec<(u16, u16)> {
    let Some(superchunk_entities) = world.entity_store().superchunk(superchunk) else {
        return Vec::new();
    };
    let CellCartesian { x: left_cell_x, y: top_cell_y } = superchunk.top_left().cartesian();
    superchunk_entities.iter().map(|entity| entity.header.stood_cell.cartesian()).map(|entity_cell| ((entity_cell.x - left_cell_x) as u16, (entity_cell.y - top_cell_y) as u16)).collect()
}
