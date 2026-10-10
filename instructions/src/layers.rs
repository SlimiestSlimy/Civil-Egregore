//! The layers a world's cells have, before any rule adds its own: what
//! a rule reads and writes by name, from the type registry
//! (`../../type_registry/`). A cell with nothing on it is dirt, which
//! has no layer.

pub use type_registry::{GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WALL_EAST, WALL_SOUTH, WET};
