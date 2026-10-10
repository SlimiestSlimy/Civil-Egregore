//! Civil Egregore's bitmap: 256 by 256 cells, and everything that can be
//! asked of them or done to them. Every layer of a chunk is one.
//!
//! The design: `docs/bitmap.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod bitmap_data;
mod bitmap_drawing;
pub mod diagnostics;
pub mod morton;
pub mod transient_data;
pub mod window;

pub use bitmap_data::{Bitmap, CellWords};

/// The bitmap is always this wide...
pub const WIDTH: usize = 256;
/// ...and this tall.
pub const HEIGHT: usize = 256;

/// Cells a word holds.
pub const BITS_PER_WORD: usize = 64;
/// Words a bitmap takes.
pub const WORDS: usize = (WIDTH * HEIGHT) / BITS_PER_WORD;
