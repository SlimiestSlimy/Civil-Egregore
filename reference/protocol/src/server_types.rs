//! The Rust types of the server's side, shared: what the server keeps
//! a world in and a rule is written with -- planes' widths, entities
//! and their attributes, simulation time, a rule's counts, a client's
//! calls. Plain types, as the game's code names them: not the types a
//! world registers (`crate::type_registry`), though they are made of
//! their IDs.

pub mod attribute_layouts;
pub mod attributes;
pub mod entity;
pub mod plane_widths;
pub mod simulation_time;
