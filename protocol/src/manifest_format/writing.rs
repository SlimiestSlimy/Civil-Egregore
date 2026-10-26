//! A [`ModDocument`] written as CDNF ([`write_mod_document`]), as
//! [`read_mod_document`](super::read_mod_document) reads it back: the
//! mod's section, its fields' list, then each type paired with its
//! fields' list. Its words are the reader's.

use super::types::{hook_word, kind_word, side_word};
use super::ModDocument;
use crate::kinds::Kind;
use cdnf::writer::{CdnfWriter, List};

/// `document` as CDNF text, as the reader reads it back.
pub fn write_mod_document(document: &ModDocument) -> String {
    let mut fields = CdnfWriter::new().section(&document.name).list().pair("side").text(side_word(document.side)).pair("version").text(&document.version);
    if document.container {
        fields = fields.text("container mod");
    }
    if let Some(hash) = &document.hash {
        fields = fields.pair("hash").text(hash);
    }
    fields = pairs_field(fields, "depends on", &document.depends_on);
    fields = pairs_field(fields, "carried dependencies", &document.carried_dependencies);
    let mut mod_section = fields.end_list();
    for each_type in &document.types {
        let mut type_fields = mod_section.pair(each_type.name()).list().pair("kind").text(kind_word(each_type.kind()));
        type_fields = match each_type.kind() {
            Kind::Plane { width_bits, tile_size } => type_fields.pair("width").text(width_bits).pair("tile size").text(tile_size),
            Kind::Entity { default_attribute: Some(attribute) } => type_fields.pair("default attribute").text(attribute),
            Kind::Attribute { size } => type_fields.pair("size").text(size),
            Kind::EntityProc { entity } => type_fields.pair("entity").text(entity),
            Kind::ScaProc { planes } => texts_field(type_fields, "planes", planes),
            Kind::RenderProc { draw_order } => type_fields.pair("draw order").text(draw_order),
            Kind::Gui { parent: Some(parent) } => type_fields.pair("parent").text(parent),
            Kind::Entity { default_attribute: None } | Kind::Gui { parent: None } | Kind::Worldgen | Kind::ClientCall | Kind::Mod(_) => type_fields,
        };
        let hooks: Vec<&str> = each_type.hooks().iter().map(|&hook| hook_word(hook)).collect();
        type_fields = texts_field(type_fields, "hooks", &hooks);
        if let Some(set_up) = each_type.set_up() {
            type_fields = type_fields.pair("set up").text(set_up);
        }
        mod_section = type_fields.end_list();
    }
    mod_section.end_section().finish()
}

/// The field `field_name` -- a list of `texts` -- written in `fields`,
/// if there is any.
fn texts_field<Then>(fields: CdnfWriter<List<Then>>, field_name: &str, texts: &[impl std::fmt::Display]) -> CdnfWriter<List<Then>> {
    let Some((first, rest)) = texts.split_first() else { return fields };
    rest.iter().fold(fields.pair(field_name).list().text(first), |list, text| list.text(text)).end_list()
}

/// The field `field_name` -- a list of `pairs`, each a name paired with
/// its text -- written in `fields`, if there is any.
fn pairs_field<Then>(fields: CdnfWriter<List<Then>>, field_name: &str, pairs: &[(String, String)]) -> CdnfWriter<List<Then>> {
    let Some(((first_name, first_text), rest)) = pairs.split_first() else { return fields };
    rest.iter().fold(fields.pair(field_name).list().pair(first_name).text(first_text), |list, (name, text)| list.pair(name).text(text)).end_list()
}
