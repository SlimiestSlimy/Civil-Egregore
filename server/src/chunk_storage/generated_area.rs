//! A superchunk's cells as its generation writes them, nothing encoded
//! yet ([`GeneratedArea`]): what storage makes an image of
//! ([`GeneratedArea::image`]). The generation is shown an area of
//! cells and writes them a cell at a time, never seeing what they are
//! kept in.

use crate::chunk_storage::layer_codec::LayerCodec;
use crate::chunk_storage::superchunk_image::{LayerChange, SuperchunkImage};
use crate::bitplane_manager::{WidePlane, Width};
use protocol::type_registry::LayerType;
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};
use utilities::coordinates::{place_from_cartesian, CellArea, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};

/// An area's cells as they are generated: of each layer, the bits of
/// its cells set so far.
pub struct GeneratedArea {
    /// The area: a superchunk's cells.
    area: CellArea,
    /// Each layer's cells, by its type number: none for one nothing is
    /// set on.
    layers: Vec<Option<GeneratedLayer>>,
}

/// One layer's cells as they are generated.
struct GeneratedLayer {
    /// Its type.
    layer_type: LayerType,
    /// Its bits' bitmaps, the lowest bit's first, each a chunk's in
    /// Morton order: none for a bit nothing is set on.
    bits: Vec<Option<Box<[CellWords]>>>,
}

impl GeneratedArea {
    /// The cells of `superchunk`, none set.
    pub fn of_superchunk(superchunk: SuperchunkIndex) -> Self {
        Self { area: CellArea { top_left: superchunk.top_left().cartesian(), side_cells: SUPERCHUNK_SIDE_CELLS }, layers: Vec::new() }
    }

    /// The area generated.
    pub fn area(&self) -> CellArea {
        self.area
    }

    /// Sets `cell` of the area on `layer_type`, a layer of a bit a cell.
    pub fn set(&mut self, layer_type: LayerType, cell: CellCartesian) {
        debug_assert!(layer_type.width_bits() == 1 && layer_type.tile_shift() == 0, "a layer of a bit a cell");
        let place = self.place_of(cell);
        self.set_bit(layer_type, 0, place);
    }

    /// Makes `value` the number of `wide_plane` at `cell` of the area --
    /// of a plane of tiles, of the tile `cell` is in.
    pub fn set_value<W: Width>(&mut self, wide_plane: WidePlane<W>, cell: CellCartesian, value: u32) {
        debug_assert!(value <= wide_plane.most_value(), "a number the plane holds");
        let layer_type = wide_plane.layer_type();
        let place = self.place_of(cell);
        // A plane of tiles keeps a tile at its place among its chunk's tiles, in Morton order.
        let kept_place = place / CELLS_IN_CHUNK * CELLS_IN_CHUNK + ((place % CELLS_IN_CHUNK) >> layer_type.tile_shift());
        for bit in 0..layer_type.width_bits() {
            if value >> bit & 1 == 1 {
                self.set_bit(layer_type, bit, kept_place);
            } else {
                self.clear_bit(layer_type, bit, kept_place);
            }
        }
    }

    /// The image of the cells set, every layer encoded by `layer_codec`.
    pub fn image(&self, layer_codec: &mut LayerCodec) -> SuperchunkImage {
        let mut encoded_layers: Vec<(usize, u64, Vec<u64>)> = Vec::new();
        for generated_layer in self.layers.iter().flatten() {
            for place in 0..CHUNKS_IN_SUPERCHUNK {
                // A plane a bit a cell, its bitmap encoded; any other, its bits' bitmaps in order.
                let encoded_layer = if generated_layer.layer_type.is_a_bitplane() {
                    generated_layer.bits[0].as_ref().map(|chunks| &chunks[place]).filter(|cells| cells.iter().any(|&word| word != 0)).map_or_else(Vec::new, |cells| layer_codec.encode_cells(cells).to_vec())
                } else {
                    crate::chunk_storage::wide::encode_bits(generated_layer.bits.iter().map(|chunks| chunks.as_ref().map(|chunks| &chunks[place])), layer_codec)
                };
                if !encoded_layer.is_empty() {
                    encoded_layers.push((place, generated_layer.layer_type.type_id(), encoded_layer));
                }
            }
        }
        let layer_changes: Vec<LayerChange> = encoded_layers.iter().map(|(place, type_id, words)| LayerChange { place: *place, type_id: *type_id, encoded_layer: words }).collect();
        SuperchunkImage::empty().rewritten_with(&layer_changes)
    }

    /// The place in the superchunk of `cell`, which is in the area.
    fn place_of(&self, cell: CellCartesian) -> usize {
        assert!(self.area.contains(cell), "a cell outside the area generated");
        place_from_cartesian(cell.x - self.area.top_left.x, cell.y - self.area.top_left.y)
    }

    /// Sets the bit `bit` of the cell at `place` of `layer_type`.
    fn set_bit(&mut self, layer_type: LayerType, bit: u32, place: usize) {
        let type_number = layer_type.type_number() as usize;
        if self.layers.len() <= type_number {
            self.layers.resize_with(type_number + 1, || None);
        }
        let generated_layer = self.layers[type_number].get_or_insert_with(|| GeneratedLayer { layer_type, bits: (0..layer_type.width_bits()).map(|_| None).collect() });
        let chunks = generated_layer.bits[bit as usize].get_or_insert_with(|| vec![[0; WORDS]; CHUNKS_IN_SUPERCHUNK].into_boxed_slice());
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        chunks[chunk][cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
    }

    /// Clears the bit `bit` of the cell at `place` of `layer_type`, if it was set.
    fn clear_bit(&mut self, layer_type: LayerType, bit: u32, place: usize) {
        if let Some(Some(generated_layer)) = self.layers.get_mut(layer_type.type_number() as usize)
            && let Some(chunks) = &mut generated_layer.bits[bit as usize]
        {
            let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
            chunks[chunk][cell / BITS_PER_WORD] &= !(1 << (cell % BITS_PER_WORD));
        }
    }
}
