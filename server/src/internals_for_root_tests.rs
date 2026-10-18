//! TEMPORARY: the server's internals the root's tests still name --
//! reached through the world's handles to its arena and its entities --
//! until the world has public reads of its own and they are rewritten
//! against them (AI.txt, LATER). Nothing but those tests uses it: the
//! root's tiers, the client's ground test and the AI's scratchpad.

pub use crate::bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp, WriteShape};
pub use crate::chunk_storage::disk;
pub use crate::chunk_storage::{LayerCodec, SuperchunkImage};
pub use crate::entity_manager::saved::{decode_state, encode_state};
pub use crate::entity_manager::{attribute_seen, find_attribute, is_attribute_list, may_be_given, push_attribute, remove_attribute, set_attribute, DefaultAttributes, EntityReader, Instructions, InstructionsApplied, WorldEntities, BLOCK_WORDS, NEVER_WAKES, WHEEL_TICKS};
pub use crate::pathfinding::Cell;
pub use crate::simulation::rules::{hot_entity_of, worldgen_of};
pub use crate::simulation::tick::{threads_for, Simulation};
// The API's own names besides, so a test's one path to the internals serves.
pub use crate::simulation::simulation_api::{compare_and_write::Compare, Attribute, AttributeBlock, AttributeType, EntityRef, EntityType, Header, LayerType, Layout, Member, SimulationTickState, TickReport, WholeAttribute};
