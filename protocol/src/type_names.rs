//! Names of mods and types, and types' full names ([`TypeName`]): its
//! mod's and its own, [`NAME_SEPARATOR`] between. A mod's name is never
//! shared in a world, a type's its mod's alone. A mod's name is its
//! crate's too, so every name is one Rust takes for a crate's.

use std::fmt;

/// Between the parts of a full name: `sheep_mod>sheep`.
pub const NAME_SEPARATOR: char = '>';

/// Whether `name` may name a mod or a type: one character or more, each
/// of `A-Z`, `a-z`, `0-9` and `_`.
pub fn is_a_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// A type, by its full name: its mod's, and its own in that mod.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeName {
    /// Its mod's name.
    pub mod_name: String,
    /// Its name in that mod.
    pub type_name: String,
}

impl fmt::Display for TypeName {
    /// `mod>type`.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}{NAME_SEPARATOR}{}", self.mod_name, self.type_name)
    }
}

impl TypeName {
    /// The type `text` names from the mod `from_mod`: its name alone, in
    /// the mod, or its mod's, [`NAME_SEPARATOR`] and its own. None if it
    /// names none.
    pub fn read(from_mod: &str, text: &str) -> Option<Self> {
        let (mod_name, type_name) = text.split_once(NAME_SEPARATOR).unwrap_or((from_mod, text));
        (is_a_name(mod_name) && is_a_name(type_name)).then(|| Self { mod_name: mod_name.to_string(), type_name: type_name.to_string() })
    }
}
