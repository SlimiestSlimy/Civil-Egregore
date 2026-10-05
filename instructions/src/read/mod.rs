//! Instructions that read: queries of a turn, which queue nothing.
//!
//! | module | what it reads |
//! |---|---|
//! | `cells` | a layer at a cell, a wide plane's number, the square about a cell, each cell sampled |
//! | `entities` | each entity waking |
//! | `around` | the 3x3 cells about a cell: a layer's, those entities stand on, one free |
//! | `area` | the 16x16 cells about a cell, and the tiles further off |
//! | `mask` | a square of a layer into masks, whole or under another |
//! | `walking` | the steps walls leave open, the step towards a goal, the nearest of a layer in reach |

pub mod area;
pub mod around;
pub mod cells;
pub mod entities;
pub mod mask;
pub mod walking;
