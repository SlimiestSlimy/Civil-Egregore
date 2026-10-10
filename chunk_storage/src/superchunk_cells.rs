//! A superchunk's cells as whoever makes them gives them, nothing
//! encoded yet: what storage makes an image of ([`SuperchunkCells::image`]).

use crate::chunk_maps::ChunkMaps;
use crate::height_map::HeightMap;
use crate::layer_codec::{LayerCodec, LayerType};
use crate::superchunk_image::{LayerChange, SuperchunkImage};
use bitmap::CellWords;

/// A superchunk's cells, not yet an image: its heights, how deep its
/// water is, and its layers' bitmaps.
pub struct SuperchunkCells {
    /// Every cell's height.
    pub heights: HeightMap,
    /// How deep the water is on every cell with any.
    pub water_depths: ChunkMaps,
    /// The layers: a chunk's place in the superchunk, a layer type and
    /// the cells set on it there -- none of them empty, none twice.
    pub layers: Vec<(usize, LayerType, CellWords)>,
}

impl SuperchunkCells {
    /// The image of these cells, every layer encoded by `codec`.
    pub fn image(&self, codec: &mut LayerCodec) -> SuperchunkImage {
        let encoded: Vec<(usize, LayerType, Vec<u64>)> = self.layers.iter().map(|(place, layer_type, cells)| (*place, *layer_type, codec.encode(cells).to_vec())).collect();
        let changes: Vec<LayerChange> = encoded.iter().map(|(place, layer_type, words)| LayerChange { place: *place, layer_type: *layer_type, encoded: words }).collect();
        SuperchunkImage::new(&self.heights).with_water(&self.water_depths).rewritten(&changes)
    }
}
