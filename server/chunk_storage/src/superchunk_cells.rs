//! A superchunk's cells as whoever makes them gives them, nothing
//! encoded yet: what storage makes an image of ([`SuperchunkCells::image`]).

use crate::chunk_maps::ChunkMaps;
use crate::layer_codec::LayerCodec;
use type_registry::LayerType;
use crate::superchunk_image::{LayerChange, SuperchunkImage};
use utilities::bitmap::CellWords;

/// A superchunk's cells, not yet an image: its heights, how deep its
/// water is, and its layers' bitmaps.
pub struct SuperchunkCells {
    /// Every cell's height, in words the base game lays out.
    pub height_words: Box<[u64]>,
    /// How deep the water is on every cell with any.
    pub water_depths: ChunkMaps,
    /// The layers: a chunk's place in the superchunk, a layer type and
    /// the cells set on it there -- none of them empty, none twice.
    pub layers: Vec<(usize, LayerType, CellWords)>,
}

impl SuperchunkCells {
    /// The image of these cells, every layer encoded by `layer_codec`.
    pub fn image(&self, layer_codec: &mut LayerCodec) -> SuperchunkImage {
        let encoded_layers: Vec<(usize, LayerType, Vec<u64>)> = self.layers.iter().map(|(place, layer_type, cells)| (*place, *layer_type, layer_codec.encode_cells(cells).to_vec())).collect();
        let layer_changes: Vec<LayerChange> = encoded_layers.iter().map(|(place, layer_type, words)| LayerChange { place: *place, layer_type: *layer_type, encoded_layer: words }).collect();
        SuperchunkImage::new(&self.height_words).with_water(&self.water_depths).rewritten_with(&layer_changes)
    }
}
