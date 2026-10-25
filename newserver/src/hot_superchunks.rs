//! The hot side: every hot superchunk in one store ([`hot_store`]), each
//! holding all it needs to be ticked ([`hot_superchunk`]) -- so far its
//! planes ([`hot_planes`]), kept sparse.

pub(crate) mod hot_store;
pub(crate) mod hot_superchunk;
pub(crate) mod hot_planes;
