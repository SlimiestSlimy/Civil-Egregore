//! A type read ([`read_type`]) from its fields, and the words of kinds,
//! sides and hooks.

use super::field_list::FieldList;
use super::{not_a_name, TypeDocument};
use cdnf::Element;
use crate::type_kinds::{Hook, Kind, Side};
use crate::simulation_time::SimulationSeconds;
use crate::type_names::{is_a_name, QualifiedTypeName, NAME_SEPARATOR};

/// Each side, and its word.
const SIDE_WORDS: [(Side, &str); 3] = [(Side::Common, "common"), (Side::Server, "server"), (Side::Client, "client")];

/// Each hook, and its words.
const HOOK_WORDS: [(Hook, &str); 3] = [(Hook::GeneratesTheWorld, "generates the world"), (Hook::KeepsTheWorldHot, "keeps the world hot"), (Hook::ShownByDefault, "shown by default")];

/// The widths a plane's number may be, in bits.
const PLANE_WIDTHS: [u32; 6] = [1, 2, 4, 8, 16, 32];

/// The largest tile scale: tiles 2^8, 256, cells a side.
const LARGEST_TILE_SCALE: u32 = 8;

/// The largest attribute, in blocks.
const LARGEST_ATTRIBUTE: u32 = 256;

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
            let tile_scale = number_of(fields.text("tile scale")?)?;
            if !PLANE_WIDTHS.contains(&width_bits) {
                return Err(format!("width: one of {PLANE_WIDTHS:?} bits, not {width_bits}"));
            }
            if tile_scale > LARGEST_TILE_SCALE {
                return Err(format!("tile scale: 0 to {LARGEST_TILE_SCALE}, tiles 2^scale cells a side, not {tile_scale}"));
            }
            Kind::Plane { width_bits, tile_scale }
        }
        "entity" => Kind::Entity { default_attribute: fields.optional_text("default attribute")?.map(reference).transpose()? },
        "attribute" => {
            let size_in_blocks = number_of(fields.text("size")?)?;
            if !(1..=LARGEST_ATTRIBUTE).contains(&size_in_blocks) {
                return Err(format!("size: 1 to {LARGEST_ATTRIBUTE} blocks, not {size_in_blocks}"));
            }
            Kind::Attribute { size_in_blocks }
        }
        "entity proc" => Kind::EntityProc { entity: reference(fields.text("entity")?)? },
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
            let seconds_per_sample = SimulationSeconds::read(seconds_text).ok_or_else(|| format!("seconds per sample: more than 0, digits and a point if wanted, not `{seconds_text}`"))?;
            Kind::ScaProc { planes, seconds_per_sample }
        }
        "worldgen" => Kind::Worldgen,
        "client call" => Kind::ClientCall,
        "render proc" => Kind::RenderProc { draw_order: number_of(fields.text("draw order")?)? },
        "gui" => Kind::Gui { parent: fields.optional_text("parent")?.map(reference).transpose()? },
        other => return Err(format!("no kind is `{other}`")),
    };
    let mut hooks = Vec::new();
    for hook_element in fields.optional_list("hooks")? {
        let Element::Text(hook_text) = hook_element else {
            return Err("hooks: a list of hooks".to_string());
        };
        let hook = word_of(&HOOK_WORDS, hook_text)?;
        if hooks.contains(&hook) || !takes_hook(&kind, hook) {
            return Err(format!("hooks: `{hook_text}` said twice, or no {}'s", kind_word(&kind)));
        }
        hooks.push(hook);
    }
    let set_up = fields.optional_text("set up")?;
    match set_up {
        Some(function_name) if !is_a_function_name(function_name) => return Err(format!("set up: `{function_name}` is no function's name: a letter or _ first, then letters, 0-9 and _")),
        None if kind.has_code() => return Err(format!("set up: missing, {}'s code its function's to make", kind_with_article(&kind))),
        _ => {}
    }
    fields.none_left(&kind_with_article(&kind))?;
    Ok(TypeDocument { name: type_name.to_string(), kind, hooks, set_up_function_name: set_up.map(str::to_string) })
}

/// Whether `text` is a function's name as Rust exports it unmangled: a
/// letter or _ first, then letters, 0-9 and _.
fn is_a_function_name(text: &str) -> bool {
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

/// The words of `hook`.
pub(super) fn hook_word(hook: Hook) -> &'static str {
    word(&HOOK_WORDS, hook)
}

/// A kind's word, as a manifest says it.
pub(super) fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::Mod(_) => "mod",
        Kind::Plane { .. } => "plane",
        Kind::Entity { .. } => "entity",
        Kind::Attribute { .. } => "attribute",
        Kind::EntityProc { .. } => "entity proc",
        Kind::ScaProc { .. } => "sca proc",
        Kind::Worldgen => "worldgen",
        Kind::ClientCall => "client call",
        Kind::RenderProc { .. } => "render proc",
        Kind::Gui { .. } => "gui",
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
        Kind::Mod(fields) => fields.side,
        Kind::Plane { .. } | Kind::Attribute { .. } | Kind::ClientCall => Side::Common,
        Kind::Entity { .. } | Kind::EntityProc { .. } | Kind::ScaProc { .. } | Kind::Worldgen => Side::Server,
        Kind::RenderProc { .. } | Kind::Gui { .. } => Side::Client,
    }
}

/// Whether a type of `kind` takes `hook`.
fn takes_hook(kind: &Kind, hook: Hook) -> bool {
    matches!((hook, kind), (Hook::GeneratesTheWorld, Kind::Worldgen) | (Hook::KeepsTheWorldHot, Kind::Entity { .. }) | (Hook::ShownByDefault, Kind::Gui { .. }))
}
