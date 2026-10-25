//! The raw compares and writes, the simulation's own: what a turn
//! queues for the tick's second phase, one function an instruction
//! -- a compare, a write, the end of a compare -- and nothing made of
//! several. The API ([`crate::simulation::simulation_api`]) is built of them; nothing
//! outside the crate reaches them.

pub(crate) mod compare;
pub(crate) mod entity;
pub(crate) mod write;
