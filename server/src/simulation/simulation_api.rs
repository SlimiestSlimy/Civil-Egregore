//! The simulation's API, what the base game is written in: every
//! read a rule makes of its turn, and every change made of several
//! compares and writes, each a function over the turn. A rule is a
//! few of them put together, and its interface to the simulation
//! ([`RuleCode`]) is here too. No type of the game is named here. A
//! module a subject -- its shape, what reads it and what writes it
//! together.

pub mod square_around_a_cell;
pub mod cell_and_its_neighbours;
pub mod cell_reads_and_writes;
pub mod compare_and_write;
pub mod entity_reads_and_writes;
pub mod tile_masks;
mod client_call_interface;
mod rule_interface;
mod worldgen_interface;
pub mod tick_and_random_stream;
// DEPRECATED: diagnostics, out when diagnostics are made again (0.8/0.9).
pub mod rule_counts;
// Composed by the actions, never by a rule: a rule asks what is about a cell, and is not shown the tiles.
pub(crate) mod four_tiles_under_the_neighbours;
pub mod steps_and_paths;

// The words the actions are asked in: where a cell and an entity
// are, what a layer and an attribute are, a turn, a tick's report, the
// lot drawn.
pub use crate::bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
pub use crate::chunk_storage::LayerType;
pub use utilities::coordinates::{CellArea, CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOUR_OFFSETS, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE_CELL};
pub use crate::entity_manager::{AttributeType, EntityBeingMade, EntityRef, EntityType, SuperchunkEntities};
pub use rule_interface::{on_each_sampled, RuleCode, RuleContext, RuleMaster, RuleRun};
pub use worldgen_interface::{Generate, SetUp, SetUpWorldgen, WorldgenCode};
pub use crate::simulation::halos::GeneratingSuperchunk;
pub use client_call_interface::{CallCode, CallContext};
pub use crate::simulation::simulation_tick_state::SimulationTickState;
pub use crate::simulation::tick::TickReport;
pub use utilities::chance::Chance;
pub use utilities::rng::RandomSource;
