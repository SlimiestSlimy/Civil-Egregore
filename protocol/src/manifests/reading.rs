//! One mod's manifest read ([`read_manifest`]) and checked as far as it
//! can be alone: its shape -- one section, the mod's, of bundles -- each
//! bundle's fields and its types', every name, each kind on its side,
//! each hook on its kind, and each library's hash. What every mod is
//! needed for -- names once among them all, dependencies, references --
//! is checked after.

use super::{Bundle, Manifest, ManifestError, Type};
use cdnf::Element;
use crate::kinds::{BundleFields, Hook, Kind, Side};
use sha2::{Digest, Sha256};
use crate::type_names::{is_a_name, BundleName, TypeName, NAME_SEPARATOR};
use std::path::{Path, PathBuf};

/// A mod's manifest, at the root of its folder.
pub const MANIFEST_FILE: &str = "manifest.cdnf";

/// Each side, and its word.
const SIDE_WORDS: [(Side, &str); 3] = [(Side::Common, "common"), (Side::Server, "server"), (Side::Client, "client")];

/// Each hook, and its words.
const HOOK_WORDS: [(Hook, &str); 3] = [(Hook::GeneratesTheWorld, "generates the world"), (Hook::KeepsTheWorldHot, "keeps the world hot"), (Hook::ShownByDefault, "shown by default")];

/// The widths a plane's number may be, in bits.
const PLANE_WIDTHS: [u32; 6] = [1, 2, 4, 8, 16, 32];

/// The most cells a plane's tile has a side.
const LARGEST_TILE: u32 = 256;

/// The largest attribute, in blocks.
const LARGEST_ATTRIBUTE: u32 = 256;

/// The manifest of the mod in `mod_folder`, read and checked alone.
pub(super) fn read_manifest(mod_folder: &Path) -> Result<Manifest, ManifestError> {
    let refused = |line: Option<usize>, section: &str, what: String| ManifestError { mod_folder: mod_folder.to_path_buf(), line, section: section.to_string(), what };
    let path = mod_folder.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|io_error| refused(None, "", format!("{}: {io_error}", path.display())))?;
    let document = cdnf::parse(&text).map_err(|cdnf_error| refused(Some(cdnf_error.line), "", cdnf_error.what))?;
    let [mod_section] = document.sections.as_slice() else {
        return Err(refused(None, "", format!("one section of one colon, the mod's: {} found", document.sections.len())));
    };
    if !document.elements.is_empty() {
        return Err(refused(None, "", "nothing before the mod's section".to_string()));
    }
    if !is_a_name(&mod_section.name) {
        return Err(refused(None, &mod_section.name, not_a_name(&mod_section.name)));
    }
    if !mod_section.elements.is_empty() {
        return Err(refused(None, &mod_section.name, "the mod's section holds its bundles alone".to_string()));
    }
    let mut bundles: Vec<Bundle> = Vec::new();
    for bundle_section in &mod_section.sections {
        let bundle = read_bundle(mod_folder, &mod_section.name, bundle_section).map_err(|(section, what)| refused(None, &section, what))?;
        if bundles.iter().any(|read_before| read_before.name == bundle.name) {
            return Err(refused(None, &bundle.name, "a bundle named twice".to_string()));
        }
        bundles.push(bundle);
    }
    if bundles.is_empty() {
        return Err(refused(None, &mod_section.name, "a mod of no bundle".to_string()));
    }
    Ok(Manifest { name: mod_section.name.clone(), folder: mod_folder.to_path_buf(), bundles })
}

/// Why `name` is none.
fn not_a_name(name: &str) -> String {
    format!("`{name}` is no name: one character or more, each of A-Z, a-z, 0-9, - and _")
}

/// The bundle `section` is, in the mod `mod_name` in `mod_folder`: or
/// the section something is wrong in, and what.
fn read_bundle(mod_folder: &Path, mod_name: &str, section: &cdnf::Section) -> Result<Bundle, (String, String)> {
    let in_bundle = |what: String| (section.name.clone(), what);
    if !is_a_name(&section.name) {
        return Err(in_bundle(not_a_name(&section.name)));
    }
    if !section.sections.is_empty() {
        return Err(in_bundle("a bundle holds no section".to_string()));
    }
    let (mut own_fields, mut types) = (None, Vec::<Type>::new());
    for element in &section.elements {
        match element {
            Element::List(items) if own_fields.is_none() => own_fields = Some(FieldList::of(items).map_err(in_bundle)?),
            Element::List(_) => return Err(in_bundle("the bundle's fields given twice".to_string())),
            Element::Pair(left, right) => {
                let (Element::Text(type_name), Element::List(items)) = (&**left, &**right) else {
                    return Err(in_bundle("a type is its name paired with the list of its fields".to_string()));
                };
                let read_type = read_type(&BundleName { mod_name: mod_name.to_string(), bundle: section.name.clone() }, type_name, items).map_err(|what| (format!("{}{NAME_SEPARATOR}{type_name}", section.name), what))?;
                if types.iter().any(|read_before| read_before.name == read_type.name) {
                    return Err(in_bundle(format!("a type named twice: {type_name}")));
                }
                types.push(read_type);
            }
            Element::Text(_) => return Err(in_bundle("text in a bundle: its fields' list, or a type -- its name paired with its fields' list".to_string())),
        }
    }
    let mut fields = own_fields.ok_or_else(|| in_bundle("no list of the bundle's fields".to_string()))?;
    let side = word_of(&SIDE_WORDS, fields.text("side").map_err(in_bundle)?).map_err(in_bundle)?;
    let library = PathBuf::from(fields.text("library").map_err(in_bundle)?);
    let library_hash_said = fields.text("hash").map_err(in_bundle)?;
    if !library_hash_said.strip_prefix("0x").is_some_and(|digits| digits.len() == 64 && digits.bytes().all(|digit| digit.is_ascii_hexdigit())) {
        return Err(in_bundle(format!("hash: 0x and 64 hexadecimal digits, not `{library_hash_said}`")));
    }
    let mut depends_on = Vec::new();
    for dependency in fields.optional_list("depends on").map_err(in_bundle)? {
        let Element::Text(dependency_text) = dependency else {
            return Err(in_bundle("depends on: a list of bundles' names".to_string()));
        };
        match BundleName::read(mod_name, dependency_text) {
            Some(bundle_name) if !depends_on.contains(&bundle_name) => depends_on.push(bundle_name),
            _ => return Err(in_bundle(format!("depends on: `{dependency_text}` names no bundle -- its name, or its mod's, {NAME_SEPARATOR} and its own -- or is said twice"))),
        }
    }
    fields.none_left("a bundle").map_err(in_bundle)?;
    if let Some(off_side) = types.iter().find(|bundle_type| side_of(&bundle_type.kind) != side) {
        return Err(in_bundle(format!("{}: a {} has no place in a bundle of the {} side", off_side.name, kind_word(&off_side.kind), word(&SIDE_WORDS, side))));
    }
    let library_path = mod_folder.join(&library);
    let library_bytes = std::fs::read(&library_path).map_err(|io_error| in_bundle(format!("library {}: {io_error}", library_path.display())))?;
    let library_hash_found = format!("0x{:x}", Sha256::digest(&library_bytes));
    if !library_hash_found.eq_ignore_ascii_case(library_hash_said) {
        return Err(in_bundle(format!("library {}: its hash is {library_hash_found}, the manifest says {library_hash_said}", library.display())));
    }
    Ok(Bundle { name: section.name.clone(), fields: BundleFields { side, library, library_hash: library_hash_found, depends_on }, types })
}

/// The type named `type_name` in the bundle `bundle_name`, its fields
/// `items`.
fn read_type(bundle_name: &BundleName, type_name: &str, items: &[Element]) -> Result<Type, String> {
    if !is_a_name(type_name) {
        return Err(not_a_name(type_name));
    }
    let mut fields = FieldList::of(items)?;
    let reference = |text: &str| type_name_of(bundle_name, text);
    let kind = match fields.text("kind")? {
        "plane" => {
            let width_bits = number_of(fields.text("width")?)?;
            let tile_size = number_of(fields.text("tile size")?)?;
            if !PLANE_WIDTHS.contains(&width_bits) {
                return Err(format!("width: one of {PLANE_WIDTHS:?} bits, not {width_bits}"));
            }
            if !tile_size.is_power_of_two() || tile_size > LARGEST_TILE {
                return Err(format!("tile size: a power of two, 1 to {LARGEST_TILE}, not {tile_size}"));
            }
            Kind::Plane { width_bits, tile_size }
        }
        "entity" => Kind::Entity { default_attribute: fields.optional_text("default attribute")?.map(reference).transpose()? },
        "attribute" => {
            let size = number_of(fields.text("size")?)?;
            if !(1..=LARGEST_ATTRIBUTE).contains(&size) {
                return Err(format!("size: 1 to {LARGEST_ATTRIBUTE} blocks, not {size}"));
            }
            Kind::Attribute { size }
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
            Kind::ScaProc { planes }
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
        None if kind.has_code() => return Err(format!("set up: missing, a {}'s code its function's to make", kind_word(&kind))),
        _ => {}
    }
    fields.none_left(&format!("a {}", kind_word(&kind)))?;
    Ok(Type { name: type_name.to_string(), kind, hooks, set_up: set_up.map(str::to_string) })
}

/// Whether `text` is a function's name as Rust exports it unmangled: a
/// letter or _ first, then letters, 0-9 and _.
fn is_a_function_name(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_') && characters.all(|each| each.is_ascii_alphanumeric() || each == '_')
}

/// The type `text` names, from the bundle `bundle_name`
/// ([`TypeName::read`]).
fn type_name_of(bundle_name: &BundleName, text: &str) -> Result<TypeName, String> {
    TypeName::read(bundle_name, text).ok_or_else(|| format!("`{text}` names no type: its name, or its bundle's and its own, or its mod's, its bundle's and its own, {NAME_SEPARATOR} between each"))
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
fn kind_word(kind: &Kind) -> &'static str {
    match kind {
        Kind::Bundle(_) => "bundle",
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

/// The side a type of `kind` is in a bundle of.
fn side_of(kind: &Kind) -> Side {
    match kind {
        Kind::Bundle(fields) => fields.side,
        Kind::Plane { .. } | Kind::Attribute { .. } | Kind::ClientCall => Side::Common,
        Kind::Entity { .. } | Kind::EntityProc { .. } | Kind::ScaProc { .. } | Kind::Worldgen => Side::Server,
        Kind::RenderProc { .. } | Kind::Gui { .. } => Side::Client,
    }
}

/// Whether a type of `kind` takes `hook`.
fn takes_hook(kind: &Kind, hook: Hook) -> bool {
    matches!((hook, kind), (Hook::GeneratesTheWorld, Kind::Worldgen) | (Hook::KeepsTheWorldHot, Kind::Entity { .. }) | (Hook::ShownByDefault, Kind::Gui { .. }))
}

/// A list of fields, each a name paired with its value, taken one by
/// one: none may be left.
struct FieldList<'document> {
    /// The fields not taken yet: each one's name and value.
    fields: Vec<(&'document str, &'document Element)>,
}

impl<'document> FieldList<'document> {
    /// The fields `items` are: each a name paired with its value, no
    /// name twice.
    fn of(items: &'document [Element]) -> Result<Self, String> {
        let mut fields = Vec::new();
        for item in items {
            let Element::Pair(left, value) = item else {
                return Err("a field is its name paired with its value".to_string());
            };
            let Element::Text(field_name) = &**left else {
                return Err("a field's name is text".to_string());
            };
            if fields.iter().any(|&(taken, _)| taken == field_name) {
                return Err(format!("{field_name}: given twice"));
            }
            fields.push((field_name.as_str(), &**value));
        }
        Ok(Self { fields })
    }

    /// The field `field_name`, taken, if there is one.
    fn take(&mut self, field_name: &str) -> Option<&'document Element> {
        let place = self.fields.iter().position(|&(each, _)| each == field_name)?;
        Some(self.fields.remove(place).1)
    }

    /// The text of the field `field_name`, if there is one.
    fn optional_text(&mut self, field_name: &str) -> Result<Option<&'document str>, String> {
        match self.take(field_name) {
            None => Ok(None),
            Some(Element::Text(text)) => Ok(Some(text)),
            Some(_) => Err(format!("{field_name}: text")),
        }
    }

    /// The text of the field `field_name`, which there must be.
    fn text(&mut self, field_name: &str) -> Result<&'document str, String> {
        self.optional_text(field_name)?.ok_or_else(|| format!("{field_name}: missing"))
    }

    /// The items of the field `field_name`'s list: none if there is no
    /// such field.
    fn optional_list(&mut self, field_name: &str) -> Result<&'document [Element], String> {
        match self.take(field_name) {
            None => Ok(&[]),
            Some(Element::List(items)) => Ok(items),
            Some(_) => Err(format!("{field_name}: a list")),
        }
    }

    /// Nothing left: or the first field `taker` takes none of.
    fn none_left(&self, taker: &str) -> Result<(), String> {
        match self.fields.first() {
            None => Ok(()),
            Some((field_name, _)) => Err(format!("{field_name}: no field {taker} takes")),
        }
    }
}
