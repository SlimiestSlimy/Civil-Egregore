//! The format every manifest-alike file shares -- a mod's manifest, its
//! premanifest -- read from its CDNF ([`read_mod_document`]): one
//! section, the mod's, of its fields' list and its types; every name,
//! each kind on its side, each hook on its kind. What one form alone
//! needs or refuses -- a manifest's hash, a premanifest's carried
//! dependencies and sources -- its own reader checks after, of the [`ModDocument`]
//! this one gives. And a [`ModDocument`] written back
//! ([`write_mod_document`]).

use crate::type_kinds::{Dependency, Hook, Kind, Side};
use crate::type_names::is_a_name;
use cdnf::Element;
use field_list::FieldList;

mod field_list;
mod type_document_reading;
mod mod_document_writing;

pub use mod_document_writing::write_mod_document;

/// The names no mod may take: the crates every mod is built and loaded
/// beside -- Rust's own, and the game's.
const TAKEN_NAMES: [&str; 8] = ["std", "core", "alloc", "proc_macro", "test", "utilities", "protocol", "cdnf"];

/// A mod as a manifest-alike file says it, read and checked as far as
/// the format can alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDocument {
    /// The mod's name: its crate's too.
    pub name: String,
    /// The side its code runs on: every type's kind on it.
    pub side: Side,
    /// Its version: any text, compared whole.
    pub version: String,
    /// Whether it is a container mod: no library, no types, only the
    /// mods it depends on.
    pub is_container: bool,
    /// The mods it depends on, each at the version it is built against:
    /// in a premanifest, those it does not carry.
    pub depends_on: Vec<DeclaredDependency>,
    /// The mods it depends on and carries in its package: a premanifest's
    /// field.
    pub carried_dependencies: Vec<DeclaredDependency>,
    /// Its library's hash, if said: SHA-256, `0x` and 64 hexadecimal
    /// digits. A manifest's field.
    pub hash: Option<String>,
    /// Its types, in the order written.
    pub types: Vec<TypeDocument>,
}

/// A mod depended on, as a document says it: its name, the version, and
/// in a premanifest, where it is packaged from -- its folder, relative to
/// the document's -- said with the version in a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredDependency {
    /// The mod's name.
    pub mod_name: String,
    /// The version depended on.
    pub version: String,
    /// Its folder, if it is packaged from its source: a premanifest's.
    pub source: Option<String>,
}

impl DeclaredDependency {
    /// The mod and version it says, its source aside.
    pub fn dependency(&self) -> Dependency {
        Dependency { mod_name: self.mod_name.clone(), version: self.version.clone() }
    }
}

/// A type: its name -- no other type's in its mod -- its kind, its
/// hooks, each one its kind takes, and its set up function's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDocument {
    /// Its name, in its mod.
    name: String,
    /// Its kind, and the kind's fields.
    kind: Kind,
    /// Its hooks.
    hooks: Vec<Hook>,
    /// The name its mod's library exports its set up function as: one a
    /// type of a kind with code has, any other may.
    set_up_function_name: Option<String>,
}

impl TypeDocument {
    /// Its name, in its mod.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its kind, and the kind's fields.
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Its hooks.
    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }

    /// The name its set up function is exported as, if it has one.
    pub fn set_up_function_name(&self) -> Option<&str> {
        self.set_up_function_name.as_deref()
    }
}

/// Why a document is no manifest-alike file: the section -- the mod, and
/// the type, as far as known -- and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// The section, empty if none.
    pub section: String,
    /// What is wrong.
    pub what: String,
}

/// The mod `document` says, as far as the format can check it alone.
pub fn read_mod_document(document: &cdnf::Document) -> Result<ModDocument, FormatError> {
    let refused = |section: &str, what: String| FormatError { section: section.to_string(), what };
    let [mod_section] = document.sections.as_slice() else {
        return Err(refused("", format!("one section of one colon, the mod's: {} found", document.sections.len())));
    };
    let name = &mod_section.name;
    if !document.elements.is_empty() {
        return Err(refused("", "nothing before the mod's section".to_string()));
    }
    if !is_a_name(name) || TAKEN_NAMES.contains(&name.as_str()) {
        return Err(refused(name, format!("{}, or one of {TAKEN_NAMES:?}", not_a_name(name))));
    }
    if !mod_section.sections.is_empty() {
        return Err(refused(name, "a mod's section holds no section".to_string()));
    }
    let in_mod = |what: String| refused(name, what);
    let (mut own_fields, mut types) = (None, Vec::<TypeDocument>::new());
    for element in &mod_section.elements {
        match element {
            Element::List(items) if own_fields.is_none() => own_fields = Some(FieldList::of(items).map_err(in_mod)?),
            Element::List(_) => return Err(in_mod("the mod's fields given twice".to_string())),
            Element::Pair(left, right) => {
                let (Element::Text(type_name), Element::List(items)) = (&**left, &**right) else {
                    return Err(in_mod("a type is its name paired with the list of its fields".to_string()));
                };
                let read_type = type_document_reading::read_type(name, type_name, items).map_err(|what| refused(&format!("{name}>{type_name}"), what))?;
                if types.iter().any(|read_before| read_before.name == read_type.name) {
                    return Err(in_mod(format!("a type named twice: {type_name}")));
                }
                types.push(read_type);
            }
            Element::Text(_) => return Err(in_mod("text in a mod: its fields' list, or a type -- its name paired with its fields' list".to_string())),
        }
    }
    let mut fields = own_fields.ok_or_else(|| in_mod("no list of the mod's fields".to_string()))?;
    let side = type_document_reading::side_named(fields.text("side").map_err(in_mod)?).map_err(in_mod)?;
    let version = fields.text("version").map_err(in_mod)?.to_string();
    let container = fields.flag("container mod");
    let depends_on = read_dependencies(fields.optional_list("depends on").map_err(in_mod)?, "depends on").map_err(in_mod)?;
    let carried_dependencies = read_dependencies(fields.optional_list("carried dependencies").map_err(in_mod)?, "carried dependencies").map_err(in_mod)?;
    if let Some(both) = carried_dependencies.iter().find(|carried| depends_on.iter().any(|depended| depended.mod_name == carried.mod_name)) {
        return Err(in_mod(format!("{}: in depends on and carried dependencies both, a carried one depended on already", both.mod_name)));
    }
    if depends_on.iter().chain(&carried_dependencies).any(|depended| depended.mod_name == *name) {
        return Err(in_mod("depends on itself".to_string()));
    }
    let hash = fields.optional_text("hash").map_err(in_mod)?;
    if let Some(hash) = hash
        && !hash.strip_prefix("0x").is_some_and(|digits| digits.len() == 64 && digits.bytes().all(|digit| digit.is_ascii_hexdigit()))
    {
        return Err(in_mod(format!("hash: 0x and 64 hexadecimal digits, not `{hash}`")));
    }
    fields.none_left("a mod").map_err(in_mod)?;
    if container && (hash.is_some() || !types.is_empty()) {
        return Err(in_mod("a container mod has no library, so no hash, and no types".to_string()));
    }
    if let Some(off_side) = types.iter().find(|each_type| type_document_reading::side_of(&each_type.kind) != side) {
        return Err(in_mod(format!("{}: {} has no place in a mod of the {} side", off_side.name, type_document_reading::kind_with_article(&off_side.kind), type_document_reading::side_word(side))));
    }
    Ok(ModDocument { name: name.clone(), side, version, is_container: container, depends_on, carried_dependencies, hash: hash.map(str::to_string), types })
}

/// The mods `items` name, in the field `field_name`, each its name
/// paired with its version -- or with a list of its version and its
/// source.
fn read_dependencies(items: &[Element], field_name: &str) -> Result<Vec<DeclaredDependency>, String> {
    let shape = || format!("{field_name}: each a mod's name paired with its version, or with a list of its version and its source");
    let mut dependencies: Vec<DeclaredDependency> = Vec::new();
    for item in items {
        let Element::Pair(left, right) = item else { return Err(shape()) };
        let Element::Text(mod_name) = &**left else { return Err(shape()) };
        if !is_a_name(mod_name) || dependencies.iter().any(|said_before| said_before.mod_name == *mod_name) {
            return Err(format!("{field_name}: `{mod_name}` is no name, or is said twice"));
        }
        let (version, source) = match &**right {
            Element::Text(version) => (version.clone(), None),
            Element::List(said) => {
                let mut said = FieldList::of(said).map_err(|what| format!("{field_name}, {mod_name}: {what}"))?;
                let version = said.text("version").map_err(|what| format!("{field_name}, {mod_name}: {what}"))?.to_string();
                let source = said.text("source").map_err(|what| format!("{field_name}, {mod_name}: {what}"))?.to_string();
                said.none_left(mod_name).map_err(|what| format!("{field_name}: {what}"))?;
                (version, Some(source))
            }
            _ => return Err(shape()),
        };
        dependencies.push(DeclaredDependency { mod_name: mod_name.clone(), version, source });
    }
    Ok(dependencies)
}

/// Why `name` is none.
fn not_a_name(name: &str) -> String {
    format!("`{name}` is no name: one character or more, each of A-Z, a-z, 0-9 and _")
}
