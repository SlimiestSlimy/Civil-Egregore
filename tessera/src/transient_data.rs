//! `transient_data/`, under the crate's folder and kept out of git:
//! what runs leave behind, one folder a kind -- measurements, worst
//! bitmaps, renders, callgrind's output.
//!
//! Function by function: `docs/lab.md`, "`transient_data.rs`".

use crate::corpus::seed::seed_in_use;
use std::path::PathBuf;
use utilities::diagnostics::table::report::Report;
use utilities::transient_data::TransientData;

/// The crate's transient data ([`utilities::transient_data`]).
pub const TRANSIENT_DATA: TransientData = TransientData::of(env!("CARGO_MANIFEST_DIR"));

/// Where the measurements are kept.
pub fn measurements() -> PathBuf {
    TRANSIENT_DATA.measurements()
}

/// Where the adversarial searches' worst bitmaps are kept.
pub fn worst() -> PathBuf {
    TRANSIENT_DATA.under("worst")
}

/// Where the renders go.
pub fn renders() -> PathBuf {
    TRANSIENT_DATA.under("renders")
}

/// Where the callgrind output goes.
pub fn callgrind() -> PathBuf {
    TRANSIENT_DATA.under("callgrind")
}

/// Notes the seed `report`'s corpus was grown from, if any was, then
/// publishes it in [`measurements`]: printed, and kept in its file.
pub fn publish(mut report: Report) {
    if let Some((seed, fresh)) = seed_in_use() {
        report.note(format!("seed {}{}", utilities::seed::hex(seed), if fresh { ", fresh for this run" } else { "" }));
    }
    TRANSIENT_DATA.publish(report);
}
