//! `transient_data/`, under the crate's folder and kept out of git:
//! `measurements/`, every measurement's latest tables, and `saves/`,
//! the tests' worlds.

use std::path::PathBuf;
use utilities::transient_data::TransientData;

/// The crate's transient data.
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));

/// Where worlds are saved, a folder each.
pub fn saves() -> PathBuf {
    TRANSIENT_DATA.under("saves")
}
