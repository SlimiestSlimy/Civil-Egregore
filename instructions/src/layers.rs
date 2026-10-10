//! The layers a world is generated with, before any rule adds its own:
//! what a rule reads and writes by name. A cell with nothing on it is
//! dirt, which has no layer.

pub use worldgen::{GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WALL_EAST, WALL_SOUTH, WET};
