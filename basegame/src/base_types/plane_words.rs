//! The base game's planes as a frame brings them (`protocol::pipe::frame`):
//! how wide a cell of each is.

use protocol::server_types::plane_widths::{Bits4, Width};

/// Bits a cell's stage takes in a frame's stages.
pub const STAGE_BITS: usize = Bits4::BITS_A_CELL as usize;
