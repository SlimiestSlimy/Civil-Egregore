//! A type read ([`read_type`]) from its fields, and the words of kinds
//! and sides.

use super::field_list::FieldList;
use super::{not_a_name, TypeDocument};
use cdnf::Element;
use crate::type_kinds::{Kind, PlaneWidth, Side, TileSide};
use crate::simulation_time::SimulationSeconds;
use crate::type_names::{is_a_name, QualifiedTypeName, NAME_SEPARATOR};

/// Each side, and its word.
const SIDE_WORDS: [(Side, &str); 3] = [(Side::Common, "common"), (Side::Server, "server"), (Side::Client, "client")];

/// The side `text` is the word of.
pub(super) fn side_named(text: &str) -> Result<Side, String> {
    word_of(&SIDE_WORDS, text)
}

/// The word of `side`.
pub(super) fn side_word(side: Side) -> &'static str {
    word(&SIDE_WORDS, side)
}

/// The type named `type_name` in the mod `mod_name`, its fields `items`.
pub(super) fn read_type(mod_name: &str, type_name: &str, items: &[Element]) -> Result<TypeDocument, String> {
    if !is_a_name(type_name) {
        return Err(not_a_name(type_name));
    }
    let mut fields = FieldList::of(items)?;
    let reference = |text: &str| type_name_of(mod_name, text);
    let kind = match fields.text("kind")? {
        "plane" => {
            let width_bits = number_of(fields.text("width")?)?;
            let side_cells = number_of(fields.text("tile side")?)?;
            if !PlaneWidth::ALL.iter().any(|width| width.bits() == width_bits) {
                return Err(format!("width: 1, 2, 4, 8, 16 or 32 bits, not {width_bits}"));
            }
            if !TileSide::ALL.iter().any(|tile_side| tile_side.side_cells() == side_cells) {
                return Err(format!("tile side: 1, 2, 4 and so on to 256 cells, not {side_cells}"));
            }
            Kind::Plane { width: PlaneWidth::of_bits(width_bits), tile_side: TileSide::of_side_cells(side_cells) }
        }
        "sca proc" => {
            let mut planes = Vec::new();
            for plane_element in fields.optional_list("planes")? {
                let Element::Text(plane_text) = plane_element else {
                    return Err("planes: a list of planes".to_string());
                };
                let plane = reference(plane_text)?;
                if planes.contains(&plane) {
                    return Err(format!("planes: {plane} said twice"));
                }
                planes.push(plane);
            }
            if planes.is_empty() {
                return Err("planes: missing, the planes it is sampled on, one or more".to_string());
            }
            let seconds_text = fields.text("seconds per sample")?;
            let mean_simulation_seconds_per_sample = SimulationSeconds::read(seconds_text).ok_or_else(|| format!("seconds per sample: more than 0, digits and a point if wanted, not `{seconds_text}`"))?;
            Kind::ScaProc { planes, mean_simulation_seconds_per_sample }
        }
        other => return Err(format!("no kind is `{other}`")),
    };
    let set_up = fields.optional_text("set up")?;
    match set_up {
        Some(static_name) if !is_an_exported_name(static_name) => return Err(format!("set up: `{static_name}` is no static's name: a letter or _ first, then letters, 0-9 and _")),
        None if kind.has_code() => return Err(format!("set up: missing, the static {}'s code is made by", kind_with_article(&kind))),
        Some(_) if !kind.has_code() => return Err(format!("set up: {} has no code to make", kind_with_article(&kind))),
        _ => {}
    }
    fields.none_left(&kind_with_article(&kind))?;
    Ok(TypeDocument { name: type_name.to_string(), kind, set_up_static_name: set_up.map(str::to_string) })
}

/// Whether `text` is a name as Rust exports one unmangled: a letter or _
/// first, then letters, 0-9 and _.
fn is_an_exported_name(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_') && characters.all(|each| each.is_ascii_alphanumeric() || each == '_')
}

/// The type `text` names, from the mod `mod_name` ([`QualifiedTypeName::read`]).
fn type_name_of(mod_name: &str, text: &str) -> Result<QualifiedTypeName, String> {
    QualifiedTypeName::read(mod_name, text).ok_or_else(|| format!("`{text}` names no type: its name, or its mod's, {NAME_SEPARATOR} and its own"))
}

/// The number `text` is.
fn number_of(text: &str) -> Result<u32, String> {
    text.parse().map_err(|_| format!("`{text}` is no number"))
}

/// What `text` is the word of among `words`.
fn word_of<T: Copy>(words: &[(T, &str)], text: &str) -> Result<T, String> {
    words.iter().find(|&&(_, word)| word == text).map(|&(meant, _)| meant).ok_or_else(|| format!("`{text}` is none of {:?}", words.iter().map(|&(_, word)| word).collect::<Vec<_>>()))
}

/// The word of `meant` among `words`.
fn word<T: PartialEq>(words: &[(T, &'static str)], meant: T) -> &'static str {
    words.iter().find(|(each, _)| *each == meant).map_or("", |&(_, word)| word)
}

/// A kind's word, as a manifest says it.
pub(super) fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::Mod { .. } => "mod",
        Kind::Plane { .. } => "plane",
        Kind::ScaProc { .. } => "sca proc",
    }
}

/// A kind's word after its article, as it is said: `an entity`, `a plane`.
pub(super) fn kind_with_article(kind: &Kind) -> String {
    let word = kind_word(kind);
    let article = if word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
    format!("{article} {word}")
}

/// The side a type of `kind` is in a mod of.
pub(super) fn side_of(kind: &Kind) -> Side {
    match kind {
        Kind::Mod { side, .. } => *side,
        Kind::Plane { .. } => Side::Common,
        Kind::ScaProc { .. } => Side::Server,
    }
}
