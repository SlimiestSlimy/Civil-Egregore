//! A superchunk's cells as its generation writes them, nothing encoded
//! yet ([`GeneratedArea`]): what storage makes an image of
//! ([`GeneratedArea::image`]). The generation is shown an area of
//! cells and writes them a cell at a time, never seeing what they are
//! kept in.

use crate::chunk_storage::layer_codec::LayerCodec;
use crate::chunk_storage::superchunk_image::{LayerChange, SuperchunkImage};
use protocol::type_registry::{LayerType, WidePlane, Width};
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};
use utilities::coordinates::{place_from_cartesian, CellArea, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};

/// An area's cells as they are generated: of each layer of a bit a cell
/// kept cold -- a wide plane's planes each one -- the cells set so far.
pub struct GeneratedArea {
    /// The area: a superchunk's cells.
    area: CellArea,
    /// Each such layer's bitmaps, a chunk each in Morton order, by its
    /// type number: none for one nothing is set on.
    bitplanes: Vec<Option<Box<[CellWords]>>>,
}

impl GeneratedArea {
    /// The cells of `superchunk`, none set.
    pub fn of_superchunk(superchunk: SuperchunkIndex) -> Self {
        Self { area: CellArea { top_left: superchunk.top_left().cartesian(), side_cells: SUPERCHUNK_SIDE_CELLS }, bitplanes: Vec::new() }
    }

    /// The area generated.
    pub fn area(&self) -> CellArea {
        self.area
    }

    /// Sets `cell` of the area on `layer_type`, a layer of a bit a cell.
    pub fn set(&mut self, layer_type: LayerType, cell: CellCartesian) {
        debug_assert!(layer_type.width_bits() == 1 && layer_type.tile_shift() == 0, "a layer of a bit a cell");
        let place = self.place_of(cell);
        self.set_bit(layer_type, place);
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
                self.set_bit(layer_type.bitplane_of(bit), kept_place);
            } else {
                self.clear_bit(layer_type.bitplane_of(bit), kept_place);
            }
        }
    }

    /// The image of the cells set, every layer encoded by `layer_codec`.
    pub fn image(&self, layer_codec: &mut LayerCodec) -> SuperchunkImage {
        let mut encoded_layers: Vec<(usize, LayerType, Vec<u64>)> = Vec::new();
        for (type_number, chunks) in self.bitplanes.iter().enumerate().filter_map(|(type_number, chunks)| Some((type_number, chunks.as_ref()?))) {
            for (place, cells) in chunks.iter().enumerate().filter(|(_, cells)| cells.iter().any(|&word| word != 0)) {
                encoded_layers.push((place, LayerType::of_a_bit(type_number as u64), layer_codec.encode_cells(cells).to_vec()));
            }
        }
        let layer_changes: Vec<LayerChange> = encoded_layers.iter().map(|(place, layer_type, words)| LayerChange { place: *place, layer_type: *layer_type, encoded_layer: words }).collect();
        SuperchunkImage::empty().rewritten_with(&layer_changes)
    }

    /// The place in the superchunk of `cell`, which is in the area.
    fn place_of(&self, cell: CellCartesian) -> usize {
        assert!(self.area.contains(cell), "a cell outside the area generated");
        place_from_cartesian(cell.x - self.area.top_left.x, cell.y - self.area.top_left.y)
    }

    /// Sets the cell at `place` of the layer `bitplane`, of a bit a cell.
    fn set_bit(&mut self, bitplane: LayerType, place: usize) {
        let type_number = bitplane.type_number() as usize;
        if self.bitplanes.len() <= type_number {
            self.bitplanes.resize_with(type_number + 1, || None);
        }
        let chunks = self.bitplanes[type_number].get_or_insert_with(|| vec![[0; WORDS]; CHUNKS_IN_SUPERCHUNK].into_boxed_slice());
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        chunks[chunk][cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
    }

    /// Clears the cell at `place` of the layer `bitplane`, if it was set.
    fn clear_bit(&mut self, bitplane: LayerType, place: usize) {
        if let Some(Some(chunks)) = self.bitplanes.get_mut(bitplane.type_number() as usize) {
            let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
            chunks[chunk][cell / BITS_PER_WORD] &= !(1 << (cell % BITS_PER_WORD));
        }
    }
}
