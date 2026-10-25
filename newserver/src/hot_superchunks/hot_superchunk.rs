//! A hot superchunk ([`HotSuperchunk`]): all a superchunk needs to be
//! ticked, held whole -- so far its planes, and its write-backs on their
//! way to the ring.

use crate::cold_storage::plane_chunk_encoding::decode_plane_chunk;
use crate::cold_storage::{BitmapCodec, PlaneChunk, ColdSuperchunk};
use crate::hot_superchunks::hot_planes::members;
use crate::hot_superchunks::hot_planes::hot_plane::HotPlane;
use protocol::type_registry::PlaneType;
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};

/// One superchunk, hot: its planes -- only those with a cell set, sorted
/// by type -- and how many of its write-backs are on their way to the
/// ring.
pub struct HotSuperchunk {
    /// Which superchunk it is.
    superchunk_index: SuperchunkIndex,
    /// Its planes with a cell set, or in the ring, sorted by type.
    planes: Vec<HotPlane>,
    /// Its write-backs taken ([`HotSuperchunk::take_dirty`]) and not yet
    /// in the ring ([`HotSuperchunk::written_back`]): while any is on its
    /// way, nothing of it is let go.
    write_backs_on_their_way: u32,
}

impl HotSuperchunk {
    /// `superchunk_index` with no cell set: what the world's generation
    /// writes into.
    pub(crate) fn new(superchunk_index: SuperchunkIndex) -> Self {
        Self { superchunk_index, planes: Vec::new(), write_backs_on_their_way: 0 }
    }

    /// `superchunk_index` as `cold superchunk` keeps it: every chunk of each of
    /// `plane_types` the cold superchunk holds, decoded with `bitmap_codec`.
    pub(crate) fn from_cold_superchunk(superchunk_index: SuperchunkIndex, plane_types: &[PlaneType], cold_superchunk: &ColdSuperchunk, bitmap_codec: &mut BitmapCodec) -> Self {
        let mut planes = Vec::new();
        for &plane_type in plane_types {
            let mut plane = HotPlane::new(plane_type);
            for chunk in 0..CHUNKS_IN_SUPERCHUNK {
                if let Some(encoded) = cold_superchunk.plane_chunk(chunk, plane_type.type_id()) {
                    plane.fill_chunk(chunk, |words| decode_plane_chunk(plane_type, encoded, bitmap_codec, words));
                }
            }
            plane.let_empty_chunks_go();
            if !plane.is_unused() {
                planes.push(plane);
            }
        }
        planes.sort_unstable_by_key(HotPlane::plane_type);
        Self { superchunk_index, planes, write_backs_on_their_way: 0 }
    }

    /// Which superchunk it is.
    pub fn superchunk_index(&self) -> SuperchunkIndex {
        self.superchunk_index
    }

    /// Its plane of `plane_type`, if it keeps one.
    #[inline]
    pub(crate) fn plane(&self, plane_type: PlaneType) -> Option<&HotPlane> {
        self.planes.binary_search_by_key(&plane_type, HotPlane::plane_type).ok().map(|plane| &self.planes[plane])
    }

    /// Its planes kept, sorted by type.
    pub(crate) fn planes(&self) -> &[HotPlane] {
        &self.planes
    }

    /// The number `plane_type` holds at `cell`: none if the cell is of
    /// another superchunk.
    pub fn value_at(&self, plane_type: PlaneType, cell: CellIndex) -> Option<u32> {
        (cell.superchunk() == self.superchunk_index).then(|| self.plane(plane_type).map_or(0, |plane| plane.value(cell.chunk().place(), cell.place())))
    }

    /// Makes `value` the number `plane_type` holds at `cell`: whether it
    /// changed -- not, if the cell is of another superchunk.
    pub fn set_value(&mut self, plane_type: PlaneType, cell: CellIndex, value: u32) -> bool {
        if cell.superchunk() != self.superchunk_index {
            return false;
        }
        let plane = match self.planes.binary_search_by_key(&plane_type, HotPlane::plane_type) {
            Ok(plane) => plane,
            Err(_) if value == 0 => return false,
            Err(plane) => {
                self.planes.insert(plane, HotPlane::new(plane_type));
                plane
            }
        };
        self.planes[plane].set_value(cell.chunk().place(), cell.place(), value)
    }

    /// Takes every dirty chunk of every plane: its numbers copied out with
    /// its plane chunk, and marked clean -- one write-back more on its way,
    /// if any. Encoded, here or off the tick, they go to
    /// [`HotSuperchunk::written_back`], in the order taken.
    pub(crate) fn take_dirty(&mut self) -> Vec<(PlaneChunk, Box<[u64]>)> {
        let mut dirty = Vec::new();
        for plane in &mut self.planes {
            for chunk in members(plane.dirty) {
                dirty.push((PlaneChunk { plane_type: plane.plane_type(), chunk: ChunkIndex::from_superchunk_place(self.superchunk_index, chunk) }, plane.chunk(chunk).into()));
            }
            plane.dirty = 0;
        }
        if !dirty.is_empty() {
            self.write_backs_on_their_way += 1;
        }
        dirty
    }

    /// A write-back taken ([`HotSuperchunk::take_dirty`]) is in the ring:
    /// its `plane_chunks` are waiting there until flushed
    /// ([`HotSuperchunk::flushed_from_ring`]).
    pub(crate) fn written_back(&mut self, plane_chunks: impl Iterator<Item = PlaneChunk>) {
        for plane_chunk in plane_chunks {
            let plane = self.planes.binary_search_by_key(&plane_chunk.plane_type, HotPlane::plane_type).expect("a plane written back is kept while on its way");
            self.planes[plane].in_ring |= 1 << plane_chunk.chunk.place();
        }
        self.write_backs_on_their_way -= 1;
        self.let_unused_go();
    }

    /// The ring has flushed every change of it into its cold superchunk: nothing of
    /// it waits there.
    pub(crate) fn flushed_from_ring(&mut self) {
        self.planes.iter_mut().for_each(|plane| plane.in_ring = 0);
        self.let_unused_go();
    }

    /// Whether its cold superchunk holds all it holds: nothing dirty, nothing on its
    /// way, nothing in the ring.
    pub(crate) fn is_written_back(&self) -> bool {
        self.write_backs_on_their_way == 0 && self.planes.iter().all(|plane| plane.dirty == 0 && plane.in_ring == 0)
    }

    /// Lets go every empty chunk and plane its cold superchunk holds as empty --
    /// nothing, while a write-back is on its way.
    fn let_unused_go(&mut self) {
        if self.write_backs_on_their_way > 0 {
            return;
        }
        self.planes.iter_mut().for_each(HotPlane::let_empty_chunks_go);
        self.planes.retain(|plane| !plane.is_unused());
    }
}
