//! A [`ModDocument`] written as CDNF ([`write_mod_document`]), as
//! [`read_mod_document`](super::read_mod_document) reads it back: the
//! mod's section, its fields' list, then each type paired with its
//! fields' list. Its words are the reader's.

use super::type_document_reading::{kind_word, side_word};
use super::{DeclaredDependency, ModDocument};
use crate::type_kinds::Kind;
use cdnf::writer::{CdnfWriter, List, PairRight};
use cdnf::CdnfError;

/// `document` as CDNF text, as the reader reads it back: or a text of it
/// the writer refused.
pub fn write_mod_document(document: &ModDocument) -> Result<String, CdnfError> {
    let mut fields = CdnfWriter::new().section(&document.name)?.list().pair("side")?.text(side_word(document.side))?.pair("version")?.text(&document.version)?;
    if document.is_container {
        fields = fields.text("container mod")?;
    }
    if let Some(hash) = &document.hash {
        fields = fields.pair("hash")?.text(hash)?;
    }
    fields = dependencies_field(fields, "depends on", &document.depends_on)?;
    fields = dependencies_field(fields, "carried dependencies", &document.carried_dependencies)?;
    let mut mod_section = fields.end_list();
    for each_type in &document.types {
        let mut type_fields = mod_section.pair(each_type.name())?.list().pair("kind")?.text(kind_word(each_type.kind()))?;
        type_fields = match each_type.kind() {
            Kind::Plane { width, tile_side } => type_fields.pair("width")?.text(width.bits())?.pair("tile side")?.text(tile_side.side_cells())?,
            Kind::ScaProc { planes, mean_simulation_seconds_per_sample } => texts_field(type_fields, "planes", planes)?.pair("seconds per sample")?.text(mean_simulation_seconds_per_sample)?,
            Kind::Mod { .. } => type_fields,
        };
        if let Some(set_up) = each_type.set_up_static_name() {
            type_fields = type_fields.pair("set up")?.text(set_up)?;
        }
        mod_section = type_fields.end_list();
    }
    Ok(mod_section.end_section().finish())
}

/// The field `field_name` -- a list of `texts` -- written in `fields`,
/// if there is any.
fn texts_field<Then>(fields: CdnfWriter<List<Then>>, field_name: &str, texts: &[impl std::fmt::Display]) -> Result<CdnfWriter<List<Then>>, CdnfError> {
    let Some((first, rest)) = texts.split_first() else { return Ok(fields) };
    let list = rest.iter().try_fold(fields.pair(field_name)?.list().text(first)?, |list, text| list.text(text))?;
    Ok(list.end_list())
}

/// The field `field_name` -- a list of `dependencies`, each a mod's
/// name paired with its version, or with its version and its source if
/// it has one -- written in `fields`, if there is any.
fn dependencies_field<Then>(fields: CdnfWriter<List<Then>>, field_name: &str, dependencies: &[DeclaredDependency]) -> Result<CdnfWriter<List<Then>>, CdnfError> {
    let Some((first, rest)) = dependencies.split_first() else { return Ok(fields) };
    let first_written = dependency_right(fields.pair(field_name)?.list().pair(&first.mod_name)?, &first.version, first.source.as_deref())?;
    let list = rest.iter().try_fold(first_written, |list, dependency| dependency_right(list.pair(&dependency.mod_name)?, &dependency.version, dependency.source.as_deref()))?;
    Ok(list.end_list())
}

/// A dependency's right side written in `right`: its version, or a list
/// of its version and its source `source`, if it has one.
fn dependency_right<Then>(right: CdnfWriter<PairRight<List<Then>>>, version: &str, source: Option<&str>) -> Result<CdnfWriter<List<Then>>, CdnfError> {
    match source {
        Some(source) => Ok(right.list().pair("version")?.text(version)?.pair("source")?.text(source)?.end_list()),
        None => right.text(version),
    }
}
