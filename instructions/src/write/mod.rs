//! Instructions that write: changes queued on a turn, applied in the
//! tick's second phase.
//!
//! | module | what it queues |
//! |---|---|
//! | `cells` | a layer set or cleared at a cell, a wide plane's number |
//! | `entities` | an entity made, put to sleep, committed as changed, removed |
//! | `mask` | a layer set or cleared under a mask |

pub mod cells;
pub mod entities;
pub mod mask;
