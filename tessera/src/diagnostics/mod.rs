//! Diagnostics: data gathered from Tessera's steps and its output, one
//! kind of data to a file. They only gather: the tests judge, and the
//! tools (`src/diagnostics/tool/`) print.
//!
//! Function by function: `docs/lab.md`, "`diagnostics/`".

pub mod adversarial;
pub mod bitmaps;
pub mod census;
pub mod examination;
pub mod measured;
pub mod png;
pub mod tool;
pub mod tree_stats;

/// The raw cells: what a bitmap costs written out, one bit a cell.
pub const RAW_CELLS: usize = bitmap::WIDTH * bitmap::HEIGHT;
