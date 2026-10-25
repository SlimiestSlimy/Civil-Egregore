//! Names of mods, bundles and types, and a type named from outside its
//! bundle ([`TypeName`]).

/// Between a bundle's name and a type's, naming the type from outside
/// its bundle: `sheep_bundle>sheep`.
pub const NAME_SEPARATOR: char = '>';

/// Whether `name` may name a mod, a bundle or a type: one character or
/// more, each of `A-Z`, `a-z`, `0-9`, `-` and `_`.
pub fn is_a_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// A type named by another: its bundle's name and its own, whichever
/// way the manifest wrote it -- alone, in the same bundle, or after its
/// bundle's and [`NAME_SEPARATOR`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeName {
    /// Its bundle's name.
    pub bundle: String,
    /// Its name in that bundle.
    pub type_name: String,
}
