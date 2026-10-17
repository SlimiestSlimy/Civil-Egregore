//! A crate's transient data: `transient_data/`, beside its `Cargo.toml`
//! and kept out of git -- what its runs leave behind
//! (`docs/utilities.md`, "Transient data").

use std::path::{Path, PathBuf};

/// The folder's name, in every crate.
pub const FOLDER: &str = "transient_data";

/// One crate's transient data.
#[derive(Clone, Copy, Debug)]
pub struct TransientData {
    /// The crate's folder: its `CARGO_MANIFEST_DIR`.
    crate_folder: &'static str,
}

impl TransientData {
    /// The transient data of the crate in `crate_folder`:
    /// `env!("CARGO_MANIFEST_DIR")`, from the crate itself.
    pub const fn for_crate(crate_folder: &'static str) -> Self {
        Self { crate_folder }
    }

    /// `relative_path`, under the crate's `transient_data/`.
    pub fn path_under(self, relative_path: &str) -> PathBuf {
        Path::new(self.crate_folder).join(FOLDER).join(relative_path)
    }
}

/// The utilities' own transient data: what their tests keep.
pub const TRANSIENT_DATA: TransientData = TransientData::for_crate(env!("CARGO_MANIFEST_DIR"));
