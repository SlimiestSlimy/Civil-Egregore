//! `transient_data/`, under the crate's folder and kept out of git: what
//! its runs leave behind ([`utilities::transient_data`]).
//!
//! | under `transient_data/` | what it holds |
//! |---|---|
//! | `worlds/` | worlds made and run by the tests, a folder each |

use std::path::PathBuf;
use utilities::transient_data::TransientData;

/// The crate's transient data.
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));

/// Where the tests' worlds go, a folder each.
pub fn worlds() -> PathBuf {
    TRANSIENT_DATA.under("worlds")
}
