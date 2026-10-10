//! `transient_data/`, under the crate's folder and kept out of git:
//! `measurements/`, every measurement's latest tables, `saves/`,
//! the tests' worlds, and `paging/`, the images running worlds have
//! paged out of memory.

use std::path::PathBuf;
use utilities::transient_data::TransientData;

/// The crate's transient data.
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));

/// Where worlds are saved, a folder each.
pub fn saves() -> PathBuf {
    TRANSIENT_DATA.under("saves")
}

/// Where running worlds page their cold pools' images, a folder each,
/// gone with its world (`chunk_storage`'s docs, "Paged to disk").
pub fn paging() -> PathBuf {
    TRANSIENT_DATA.under("paging")
}
