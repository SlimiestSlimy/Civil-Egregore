//! A mod's premanifest read and checked ([`read_premanifest`]): the
//! manifest its developer writes, every field but what the packager
//! finds out -- its library's hash -- with the dependencies it carries
//! said apart from those it does not. Read as CDNF, then as the format
//! every manifest-alike file shares, then as a premanifest: every check
//! one mod can have alone, before anything is built. What comes out
//! ([`Premanifest`]) only this module makes.

use protocol::manifest_format::{read_mod_document, FormatError, ModDocument};
use std::path::Path;

/// A mod's premanifest, at the root of its crate.
pub const PREMANIFEST_FILE: &str = "premanifest.cdnf";

/// The premanifest of the mod whose crate is `mod_folder`, read and
/// checked alone: or what is wrong, and where.
pub fn read_premanifest(mod_folder: &Path) -> Result<Premanifest, String> {
    let path = mod_folder.join(PREMANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|io_error| format!("{}: {io_error}", path.display()))?;
    let document = cdnf::parse(&text).map_err(|cdnf_error| format!("{}, line {}: {}", path.display(), cdnf_error.line, cdnf_error.what))?;
    let document = read_mod_document(&document).map_err(|FormatError { section, what }| format!("{}, {section}: {what}", path.display()))?;
    if document.hash.is_some() {
        return Err(format!("{}, {}: hash: the packager's to write, in the manifest", path.display(), document.name));
    }
    Ok(Premanifest { document })
}

/// A mod's premanifest, checked alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Premanifest {
    /// The mod as its premanifest says it: no hash.
    document: ModDocument,
}

impl Premanifest {
    /// The mod as its premanifest says it.
    pub fn document(&self) -> &ModDocument {
        &self.document
    }

    /// The manifest it makes, its library's hash `hash`: every field
    /// its but its dependencies' sources, the dependencies it carries
    /// among those it depends on.
    pub fn manifest(&self, hash: Option<String>) -> ModDocument {
        let mut manifest = self.document.clone();
        manifest.depends_on.append(&mut manifest.carried_dependencies);
        manifest.depends_on.iter_mut().for_each(|dependency| dependency.source = None);
        manifest.hash = hash;
        manifest
    }
}
