//! Names of mods, bundles and types, and their full names: a bundle's
//! its mod's and its own ([`BundleName`]), a type's its bundle's and
//! its own ([`TypeName`]), each part after the one before and
//! [`NAME_SEPARATOR`]. A mod's name is the widest, never shared; a
//! bundle's is its mod's alone, a type's its bundle's alone.

use std::fmt;

/// Between the parts of a full name: `sheep_mod>sheep_bundle>sheep`.
pub const NAME_SEPARATOR: char = '>';

/// Whether `name` may name a mod, a bundle or a type: one character or
/// more, each of `A-Z`, `a-z`, `0-9`, `-` and `_`.
pub fn is_a_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// A bundle, by its full name: its mod's, and its own in that mod.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BundleName {
    /// Its mod's name.
    pub mod_name: String,
    /// Its name in that mod.
    pub bundle: String,
}

/// A type, by its full name: its bundle's, and its own in that bundle.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeName {
    /// Its bundle's full name.
    pub bundle_name: BundleName,
    /// Its name in that bundle.
    pub type_name: String,
}

impl fmt::Display for BundleName {
    /// `mod>bundle`.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}{NAME_SEPARATOR}{}", self.mod_name, self.bundle)
    }
}

impl fmt::Display for TypeName {
    /// `mod>bundle>type`.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}{NAME_SEPARATOR}{}", self.bundle_name, self.type_name)
    }
}

impl BundleName {
    /// The bundle `text` names from the mod `mod_name`: its name alone,
    /// in the mod, or its mod's, [`NAME_SEPARATOR`] and its own. None if
    /// it names none.
    pub fn read(mod_name: &str, text: &str) -> Option<Self> {
        let (mod_part, bundle) = text.split_once(NAME_SEPARATOR).unwrap_or((mod_name, text));
        (is_a_name(mod_part) && is_a_name(bundle)).then(|| Self { mod_name: mod_part.to_string(), bundle: bundle.to_string() })
    }
}

impl TypeName {
    /// The type `text` names from the bundle `from`: its name alone, in
    /// the bundle; its bundle's and its own, in the mod; or its mod's,
    /// its bundle's and its own, [`NAME_SEPARATOR`] between each. None
    /// if it names none.
    pub fn read(from: &BundleName, text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(NAME_SEPARATOR).collect();
        let (mod_name, bundle, type_name) = match parts.as_slice() {
            [type_name] => (from.mod_name.as_str(), from.bundle.as_str(), *type_name),
            [bundle, type_name] => (from.mod_name.as_str(), *bundle, *type_name),
            [mod_name, bundle, type_name] => (*mod_name, *bundle, *type_name),
            _ => return None,
        };
        (is_a_name(mod_name) && is_a_name(bundle) && is_a_name(type_name)).then(|| Self { bundle_name: BundleName { mod_name: mod_name.to_string(), bundle: bundle.to_string() }, type_name: type_name.to_string() })
    }
}
