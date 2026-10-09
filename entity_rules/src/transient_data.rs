//! `transient_data/`, under the crate's folder and kept out of git: what
//! its runs leave behind (`utilities::transient_data`).
//!
//! Nothing is kept there yet.

use instructions::handed_on::TransientData;

/// The crate's transient data.
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));
