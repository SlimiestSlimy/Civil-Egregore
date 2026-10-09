//! What the crates under the instructions have that a rule names,
//! handed on: where a cell and an entity are, what a layer and an
//! attribute are, the world a rule is ticked on and the lot it draws.
//! A rule depends on the instructions and on nothing else; what it
//! needs of another crate it has from here, or it is added here.

pub use bitplane_manager::{BitmapArena, Shape, Write, WriteOp};
pub use chunk_storage::mock::{grass_on_dirt, DIRT, GRASS};
pub use chunk_storage::{Bits4, ChunkStorage, LayerCodec, LayerType, Wide};
pub use coordinates::{square_from_middle, CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, NEIGHBOURS, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
pub use entity_manager::{Attribute, AttributeType, Entities, EntityEdit, EntityId, EntityRef, EntityType, Header};
pub use utilities::rng::Rng;
pub use utilities::transient_data::TransientData;
pub use worldgen::{WALL_EAST, WALL_SOUTH, WET};
