//! The hot store ([`HotStore`]): every hot superchunk, one sorted list
//! behind one handle, and the world's plane types.

use crate::hot_superchunks::hot_superchunk::HotSuperchunk;
use crate::hot_superchunks::hot_planes::hot_plane_reads::HotPlaneReader;
use protocol::type_registry::PlaneType;
use utilities::coordinates::SuperchunkIndex;

/// Every hot superchunk, sorted by superchunk index, and the plane types
/// of the world -- what a plane named by its ID alone is found by.
pub struct HotStore {
    /// The hot superchunks, sorted by superchunk index.
    superchunks: Vec<HotSuperchunk>,
    /// Every plane type of the world, sorted by ID.
    plane_types: Vec<PlaneType>,
}

impl HotStore {
    /// No superchunk hot, in a world of `plane_types`.
    pub(crate) fn new(mut plane_types: Vec<PlaneType>) -> Self {
        plane_types.sort_unstable_by_key(|plane_type| plane_type.type_id());
        plane_types.dedup();
        Self { superchunks: Vec::new(), plane_types }
    }

    /// Every plane type of the world, sorted by ID.
    pub(crate) fn plane_types(&self) -> &[PlaneType] {
        &self.plane_types
    }

    /// The plane type of the ID `type_id`, if the world has one: an
    /// instruction names its plane by its ID alone.
    pub(crate) fn plane_type_of_id(&self, type_id: u64) -> Option<PlaneType> {
        self.plane_types.binary_search_by_key(&type_id, |plane_type| plane_type.type_id()).ok().map(|place| self.plane_types[place])
    }

    /// Every hot superchunk, sorted by superchunk index.
    pub(crate) fn superchunks(&self) -> &[HotSuperchunk] {
        &self.superchunks
    }

    /// Every hot superchunk, sorted by superchunk index, to change each
    /// apart from the others: none added or taken through it.
    pub(crate) fn superchunks_mut(&mut self) -> &mut [HotSuperchunk] {
        &mut self.superchunks
    }

    /// The hot superchunk `superchunk`, if it is hot.
    pub(crate) fn superchunk(&self, superchunk: SuperchunkIndex) -> Option<&HotSuperchunk> {
        self.place_of(superchunk).ok().map(|place| &self.superchunks[place])
    }

    /// The hot superchunk `superchunk`, if it is hot, to change.
    pub(crate) fn superchunk_mut(&mut self, superchunk: SuperchunkIndex) -> Option<&mut HotSuperchunk> {
        self.place_of(superchunk).ok().map(|place| &mut self.superchunks[place])
    }

    /// Makes `hot_superchunk` hot, in its place: it was not.
    pub(crate) fn adopt(&mut self, hot_superchunk: HotSuperchunk) {
        let place = self.place_of(hot_superchunk.superchunk_index()).expect_err("a superchunk is made hot once");
        self.superchunks.insert(place, hot_superchunk);
    }

    /// A reader of every hot superchunk's planes.
    pub(crate) fn reader(&self) -> HotPlaneReader<'_> {
        HotPlaneReader::new(&self.superchunks)
    }

    /// Where `superchunk` is among the hot ones, or would go.
    fn place_of(&self, superchunk: SuperchunkIndex) -> Result<usize, usize> {
        self.superchunks.binary_search_by_key(&superchunk, HotSuperchunk::superchunk_index)
    }
}
