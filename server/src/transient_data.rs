//! `transient_data/`, under the crate's folder and kept out of git: what
//! its runs leave behind ([`utilities::transient_data`]).
//!
//! | under `transient_data/` | what it holds |
//! |---|---|
//! | `measurements/` | every measurement's latest tables, as CSV |
//! | `saves/` | worlds saved, a folder each: the tests' |

use std::path::PathBuf;
use utilities::transient_data::TransientData;

/// The crate's transient data.
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));

/// Where worlds are saved, a folder each.
pub fn saves() -> PathBuf {
    TRANSIENT_DATA.under("saves")
}
